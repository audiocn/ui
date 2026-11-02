import { act, renderHook, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AnalyserInput } from "@/hooks/use-audio-analyser";
import { AudioContextProvider } from "@/hooks/use-audio-context";
import { useVuNeedle } from "@/hooks/use-vu-needle";
import type { VuNeedleOptions } from "@/hooks/use-vu-needle";
import { createFakeAudioContext } from "@/test/fake-audio";

const { load } = vi.hoisted(() => ({
  load: vi.fn<() => Promise<void>>(),
}));

vi.mock("@/lib/audio/vu-worklet", () => ({
  VU_PROCESSOR_NAME: "test-vu",
  loadVuProcessor: load,
}));

const PARAM_NAMES = [
  "lowPassCutoffHz",
  "lowPassQ",
  "rebound",
  "referenceDb",
  "minDb",
  "maxDb",
] as const;

class FakeWorkletNode extends EventTarget {
  static nodes: FakeWorkletNode[] = [];
  readonly options: AudioWorkletNodeOptions;
  readonly port = Object.assign(new EventTarget(), {
    close: vi.fn(),
    postMessage: vi.fn(),
    start: vi.fn(),
  });
  readonly parameters = new Map(
    PARAM_NAMES.map((name) => [name, { setValueAtTime: vi.fn() }])
  );
  readonly connect = vi.fn();
  readonly disconnect = vi.fn();
  constructor(
    _context: BaseAudioContext,
    _name: string,
    options: AudioWorkletNodeOptions
  ) {
    super();
    this.options = options;
    FakeWorkletNode.nodes.push(this);
  }
  post(position: number) {
    this.port.dispatchEvent(new MessageEvent("message", { data: position }));
  }
}

interface Props {
  input: AnalyserInput;
  options?: VuNeedleOptions;
}

const setup = () => {
  const audio = createFakeAudioContext();
  const onPosition = vi.fn<(position: number) => void>();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <StrictMode>
      <AudioContextProvider context={audio.context}>
        {children}
      </AudioContextProvider>
    </StrictMode>
  );
  const render = (initialProps: Props) =>
    renderHook(
      ({ input, options }: Props) => useVuNeedle(input, onPosition, options),
      { initialProps, wrapper }
    );
  return { audio, onPosition, render };
};

const inputNode = () =>
  ({ connect: vi.fn(), disconnect: vi.fn() }) as unknown as AudioNode & {
    connect: ReturnType<typeof vi.fn>;
    disconnect: ReturnType<typeof vi.fn>;
  };

const liveNodes = () =>
  FakeWorkletNode.nodes.filter(
    (node) => node.disconnect.mock.calls.length === 0
  );

describe("useVuNeedle", () => {
  beforeEach(() => {
    load.mockReset().mockResolvedValue();
    FakeWorkletNode.nodes = [];
    vi.stubGlobal("AudioWorkletNode", FakeWorkletNode);
    // jsdom has no MediaStream; the hook only checks instanceof.
    vi.stubGlobal("MediaStream", vi.fn());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("stays idle with no input", () => {
    const { render } = setup();
    const { result } = render({ input: null });
    expect(result.current.status).toBe("idle");
    expect(load).not.toHaveBeenCalled();
  });

  it("taps the input with one silent node, kept running through the destination", async () => {
    const { audio, render } = setup();
    const input = inputNode();
    const { result } = render({ input });
    await waitFor(() => expect(result.current.status).toBe("running"));
    const [node, ...extra] = liveNodes();
    expect(extra).toHaveLength(0);
    expect(input.connect).toHaveBeenCalledWith(node);
    expect(node?.connect).toHaveBeenCalledWith(audio.fake.destination);
    expect(node?.options).toMatchObject({
      channelCount: 2,
      channelCountMode: "explicit",
      channelInterpretation: "speakers",
      processorOptions: { channel: "max" },
    });
  });

  it("sends needle and scale settings as params, defaults included", async () => {
    const { render } = setup();
    const { result } = render({
      input: inputNode(),
      options: { lowPassCutoffHz: 3, referenceDb: -20 },
    });
    await waitFor(() => expect(result.current.status).toBe("running"));
    const [node] = liveNodes();
    expect(node?.options.parameterData).toEqual({
      lowPassCutoffHz: 3,
      lowPassQ: 0.6,
      maxDb: 3,
      minDb: -10,
      rebound: 0.5,
      referenceDb: -20,
    });
  });

  it("changes params in place, keeping the needle's place", async () => {
    const { audio, render } = setup();
    audio.clock.now = 2;
    const input = inputNode();
    const { rerender, result } = render({ input });
    await waitFor(() => expect(result.current.status).toBe("running"));
    const [node] = liveNodes();
    rerender({ input, options: { lowPassCutoffHz: 3 } });
    expect(liveNodes()).toEqual([node]);
    expect(
      node?.parameters.get("lowPassCutoffHz")?.setValueAtTime
    ).toHaveBeenLastCalledWith(3, 2);
  });

  it("builds a new node for another channel", async () => {
    const { render } = setup();
    const input = inputNode();
    const { rerender, result } = render({ input });
    await waitFor(() => expect(result.current.status).toBe("running"));
    const [first] = liveNodes();
    rerender({ input, options: { channel: 1 } });
    await waitFor(() => expect(liveNodes()).not.toContain(first));
    expect(liveNodes()).toHaveLength(1);
    expect(liveNodes()[0]?.options.processorOptions).toMatchObject({
      channel: 1,
    });
  });

  it("passes positions from the audio thread to the callback", async () => {
    const { onPosition, render } = setup();
    const { result } = render({ input: inputNode() });
    await waitFor(() => expect(result.current.status).toBe("running"));
    act(() => liveNodes()[0]?.post(0.62));
    expect(onPosition).toHaveBeenLastCalledWith(0.62);
  });

  it("sends the needle back to rest on reset", async () => {
    const { render } = setup();
    const { result } = render({ input: inputNode() });
    await waitFor(() => expect(result.current.status).toBe("running"));
    result.current.reset();
    expect(liveNodes()[0]?.port.postMessage).toHaveBeenCalledWith("reset", []);
  });

  it("releases only its own connection on unmount", async () => {
    const { audio, render } = setup();
    const input = inputNode();
    const { result, unmount } = render({ input });
    await waitFor(() => expect(result.current.status).toBe("running"));
    const [node] = liveNodes();
    unmount();
    expect(input.disconnect).toHaveBeenCalledWith(node);
    expect(input.disconnect).not.toHaveBeenCalledWith();
    expect(node?.disconnect).toHaveBeenCalled();
    expect(node?.port.postMessage).toHaveBeenCalledWith("stop", []);
    expect(audio.fake.destination.disconnect).not.toHaveBeenCalled();
  });

  it("makes and releases its own source for a stream", async () => {
    const { audio, render } = setup();
    const stream = new MediaStream();
    const { result, unmount } = render({ input: stream });
    await waitFor(() => expect(result.current.status).toBe("running"));
    expect(audio.fake.createMediaStreamSource).toHaveBeenCalledWith(stream);
    const source = audio.fake.createMediaStreamSource.mock.results.at(-1)
      ?.value as { disconnect: ReturnType<typeof vi.fn> };
    unmount();
    expect(source.disconnect).toHaveBeenCalled();
  });

  it("reports a crash on the audio thread", async () => {
    const { render } = setup();
    const { result } = render({ input: inputNode() });
    await waitFor(() => expect(result.current.status).toBe("running"));
    const [node] = liveNodes();
    act(() => node?.dispatchEvent(new Event("processorerror")));
    expect(result.current.status).toBe("error");
    expect(result.current.error?.message).toMatch(/audio thread/u);
    expect(node?.disconnect).toHaveBeenCalled();
  });

  it("starts with the params set while the processor loaded", async () => {
    const { render } = setup();
    const input = inputNode();
    const { rerender, result } = render({ input });
    // The loader resolves on a later microtask, so this lands mid-load.
    rerender({ input, options: { lowPassCutoffHz: 3 } });
    expect(liveNodes()).toHaveLength(0);
    await waitFor(() => expect(result.current.status).toBe("running"));
    expect(liveNodes()[0]?.options.parameterData).toMatchObject({
      lowPassCutoffHz: 3,
    });
  });

  it("moves to a new input, releasing only the old connection", async () => {
    const { render } = setup();
    const first = inputNode();
    const second = inputNode();
    const { rerender, result } = render({ input: first });
    await waitFor(() => expect(result.current.status).toBe("running"));
    const [old] = liveNodes();
    rerender({ input: second });
    await waitFor(() => expect(liveNodes()).not.toContain(old));
    const [next] = liveNodes();
    expect(first.disconnect).toHaveBeenCalledWith(old);
    expect(second.connect).toHaveBeenCalledWith(next);
    expect(liveNodes()).toHaveLength(1);
  });

  it("reports a suspended context", async () => {
    const { audio, render } = setup();
    audio.fake.state = "suspended";
    const { result } = render({ input: inputNode() });
    await waitFor(() => expect(result.current.status).toBe("suspended"));
  });

  it("reports a closed context as an error", () => {
    const { audio, render } = setup();
    audio.fake.state = "closed";
    const { result } = render({ input: inputNode() });
    expect(result.current.status).toBe("error");
    expect(result.current.error?.message).toMatch(/closed/u);
  });

  it("reports a browser without Web Audio as an error", () => {
    const { result } = renderHook(() => useVuNeedle(inputNode(), vi.fn()));
    expect(result.current.status).toBe("error");
    expect(result.current.error?.message).toMatch(/Web Audio/u);
  });

  it("reports a processor that fails to load", async () => {
    load.mockRejectedValue(new Error("no worklets here"));
    const { render } = setup();
    const { result } = render({ input: inputNode() });
    await waitFor(() => expect(result.current.status).toBe("error"));
    expect(result.current.error?.message).toBe("no worklets here");
  });
});
