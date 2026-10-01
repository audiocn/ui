import { renderHook } from "@testing-library/react";
import { Activity } from "react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { AudioContextProvider } from "@/hooks/use-audio-context";
import { useGainNode } from "@/hooks/use-gain-node";
import type { UseGainNodeOptions } from "@/hooks/use-gain-node";
import { createFakeAudioContext } from "@/test/fake-audio";

type Mode = "visible" | "hidden";

const setup = () => {
  const audio = createFakeAudioContext();
  const activity: { mode: Mode } = { mode: "visible" };
  const wrapper = ({ children }: { children: ReactNode }) => (
    <AudioContextProvider context={audio.context}>
      <Activity mode={activity.mode}>{children}</Activity>
    </AudioContextProvider>
  );
  const render = (initialProps: UseGainNodeOptions) =>
    renderHook((props: UseGainNodeOptions) => useGainNode(props), {
      initialProps,
      wrapper,
    });
  return { activity, audio, render };
};

describe("useGainNode", () => {
  beforeEach(() => {
    // jsdom has no MediaStream; the hook only checks instanceof.
    vi.stubGlobal(
      "MediaStream",
      class {
        readonly id = "stream";
      }
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("starts at its gain, then ramps later changes", () => {
    const { audio, render } = setup();
    const { rerender } = render({ destination: null, gain: 0 });
    const [node] = audio.gains;
    expect(node?.gain.setValueAtTime).toHaveBeenCalledWith(0, 0);
    expect(node?.gain.setTargetAtTime).not.toHaveBeenCalled();

    rerender({ destination: null, gain: 0.5 });
    expect(node?.gain.setTargetAtTime).toHaveBeenCalledWith(0.5, 0, 0.01);
  });

  it("wires a stream to the speakers and undoes it on unmount", () => {
    const { audio, render } = setup();
    const stream = new MediaStream();
    const { result, unmount } = render({ input: stream });
    const node = result.current;
    const source = audio.fake.createMediaStreamSource.mock.results[0]
      ?.value as {
      connect: ReturnType<typeof vi.fn>;
      disconnect: ReturnType<typeof vi.fn>;
    };
    expect(audio.fake.createMediaStreamSource).toHaveBeenCalledWith(stream);
    expect(source.connect).toHaveBeenCalledWith(node);
    expect(node?.connect).toHaveBeenCalledWith(audio.fake.destination);

    unmount();
    expect(source.disconnect).toHaveBeenCalled();
    expect(node?.disconnect).toHaveBeenCalledWith(audio.fake.destination);
  });

  it("routes nowhere with a null destination", () => {
    const { render } = setup();
    const { result } = render({ destination: null });
    expect(result.current?.connect).not.toHaveBeenCalled();
  });

  it("disconnects on an Activity hide and reconnects on show", () => {
    const { activity, audio, render } = setup();
    const { rerender, result } = render({ gain: 0.25 });
    const node = result.current;
    activity.mode = "hidden";
    rerender({ gain: 0.25 });
    expect(node?.disconnect).toHaveBeenCalledWith(audio.fake.destination);
    activity.mode = "visible";
    rerender({ gain: 0.25 });
    expect(node?.connect).toHaveBeenCalledTimes(2);
  });
});
