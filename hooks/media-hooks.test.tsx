import { act, renderHook, waitFor } from "@testing-library/react";
import { Activity } from "react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { AudioContextProvider } from "@/hooks/use-audio-context";
import { useAudioPlayer } from "@/hooks/use-audio-player";
import type { UseAudioPlayerOptions } from "@/hooks/use-audio-player";
import { useMicrophone } from "@/hooks/use-microphone";
import { useSound } from "@/hooks/use-sound";
import { useSystemAudio } from "@/hooks/use-system-audio";
import { advance, useFakeFrames } from "@/test/fake-frames";

type Mode = "visible" | "hidden";

/** A renderHook wrapper whose Activity mode the test flips between renders. */
const createActivity = () => {
  const activity: { mode: Mode } = { mode: "visible" };
  const wrapper = ({ children }: { children: ReactNode }) => (
    <Activity mode={activity.mode}>{children}</Activity>
  );
  return { activity, wrapper };
};

const fakeStream = () => {
  const track = { addEventListener: vi.fn(), stop: vi.fn() };
  const stream = {
    getAudioTracks: () => [track],
    getTracks: () => [track],
    getVideoTracks: () => [],
  } as unknown as MediaStream;
  return { stream, track };
};

const setMediaDevices = (devices: Partial<MediaDevices>) => {
  Object.defineProperty(navigator, "mediaDevices", {
    configurable: true,
    value: {
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      ...devices,
    },
  });
};

const fakeNode = () => ({ connect: vi.fn(), disconnect: vi.fn() });

const renderPlayer = (initialProps: UseAudioPlayerOptions) => {
  const { activity, wrapper } = createActivity();
  const view = renderHook(
    (props: UseAudioPlayerOptions) => useAudioPlayer(props),
    { initialProps, wrapper }
  );
  return { activity, ...view };
};

describe("useMicrophone", () => {
  it("doesn't hand out the stopped stream when started again", async () => {
    const first = fakeStream();
    const getUserMedia = vi.fn(() => Promise.resolve(first.stream));
    setMediaDevices({ getUserMedia });
    const { result } = renderHook(() => useMicrophone({ enabled: true }));
    await waitFor(() => {
      expect(result.current.status).toBe("active");
    });

    act(() => {
      result.current.stop();
    });
    expect(first.track.stop).toHaveBeenCalled();

    const second = Promise.withResolvers<MediaStream>();
    getUserMedia.mockImplementation(() => second.promise);
    await act(async () => {
      await result.current.start();
    });
    expect(result.current.status).toBe("acquiring");
    expect(result.current.stream).toBeNull();
  });
});

describe("useAudioPlayer", () => {
  let load: ReturnType<typeof vi.spyOn>;
  let pause: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    load = vi
      .spyOn(HTMLMediaElement.prototype, "load")
      .mockImplementation(() => {
        // jsdom has no media pipeline.
      });
    pause = vi
      .spyOn(HTMLMediaElement.prototype, "pause")
      .mockImplementation(() => {
        // jsdom has no media pipeline.
      });
    vi.spyOn(HTMLMediaElement.prototype, "play").mockResolvedValue();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("keeps a volume set imperatively when another prop changes", () => {
    const { result, rerender } = renderPlayer({ muted: false, src: "a.mp3" });
    act(() => {
      result.current.setVolume(0.3);
    });
    rerender({ muted: true, src: "a.mp3" });
    expect(result.current.element?.volume).toBeCloseTo(0.3);
    expect(result.current.element?.muted).toBe(true);
  });

  it("doesn't reload the track when autoPlay or preload changes", () => {
    const { rerender } = renderPlayer({ autoPlay: false, src: "a.mp3" });
    const loads = load.mock.calls.length;
    rerender({ autoPlay: true, src: "a.mp3" });
    rerender({ autoPlay: true, preload: "auto", src: "a.mp3" });
    expect(load.mock.calls.length).toBe(loads);
  });

  it("stops the old track when the source is cleared", () => {
    const { result, rerender } = renderPlayer({ src: "a.mp3" });
    const loads = load.mock.calls.length;
    rerender({});
    expect(result.current.element?.hasAttribute("src")).toBe(false);
    expect(load.mock.calls.length).toBe(loads + 1);
  });

  it("pauses on an Activity hide and comes back paused, not reloaded", () => {
    const { activity, result, rerender } = renderPlayer({ src: "a.mp3" });
    act(() => {
      result.current.element?.dispatchEvent(new Event("playing"));
    });
    expect(result.current.status).toBe("playing");
    const loads = load.mock.calls.length;

    activity.mode = "hidden";
    rerender({ src: "a.mp3" });
    expect(pause).toHaveBeenCalled();
    activity.mode = "visible";
    rerender({ src: "a.mp3" });
    expect(result.current.status).toBe("paused");
    expect(load.mock.calls.length).toBe(loads);
  });
});

describe("useSystemAudio", () => {
  beforeEach(() => {
    // jsdom has no MediaStream; the hook wraps the picked audio tracks in one.
    vi.stubGlobal(
      "MediaStream",
      class {
        readonly tracks: MediaStreamTrack[];
        constructor(tracks: MediaStreamTrack[]) {
          this.tracks = tracks;
        }
        getTracks() {
          return this.tracks;
        }
      }
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("drops a capture that arrives after stop()", async () => {
    const picked = Promise.withResolvers<MediaStream>();
    setMediaDevices({ getDisplayMedia: vi.fn(() => picked.promise) });
    const { stream, track } = fakeStream();
    const { result } = renderHook(() => useSystemAudio());

    let started: Promise<void> | undefined;
    act(() => {
      started = result.current.start();
    });
    expect(result.current.status).toBe("prompting");
    act(() => {
      result.current.stop();
    });
    await act(async () => {
      picked.resolve(stream);
      await started;
    });

    expect(track.stop).toHaveBeenCalled();
    expect(result.current.status).toBe("idle");
    expect(result.current.stream).toBeNull();
  });

  it("comes back idle after an Activity hide, not active with a dead stream", async () => {
    const { stream, track } = fakeStream();
    setMediaDevices({ getDisplayMedia: vi.fn(() => Promise.resolve(stream)) });
    const { activity, wrapper } = createActivity();
    const { result, rerender } = renderHook(() => useSystemAudio(), {
      wrapper,
    });
    await act(async () => {
      await result.current.start();
    });
    expect(result.current.status).toBe("active");

    activity.mode = "hidden";
    rerender();
    expect(track.stop).toHaveBeenCalled();
    activity.mode = "visible";
    rerender();
    expect(result.current.status).toBe("idle");
    expect(result.current.stream).toBeNull();
  });
});

describe("useSound", () => {
  beforeEach(() => {
    useFakeFrames();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("reports progress from the voice, not from props changed mid-play", () => {
    const clock = { now: 0 };
    const context = {
      addEventListener: vi.fn(),
      createBufferSource: () => ({
        ...fakeNode(),
        addEventListener: vi.fn(),
        buffer: null,
        loop: false,
        playbackRate: { value: 1 },
        start: vi.fn(),
        stop: vi.fn(),
      }),
      createGain: () => ({
        ...fakeNode(),
        gain: { setTargetAtTime: vi.fn(), setValueAtTime: vi.fn(), value: 1 },
      }),
      get currentTime() {
        return clock.now;
      },
      destination: {},
      removeEventListener: vi.fn(),
      state: "running",
    } as unknown as AudioContext;
    const buffer = { duration: 1 } as AudioBuffer;
    const wrapper = ({ children }: { children: ReactNode }) => (
      <AudioContextProvider context={context}>{children}</AudioContextProvider>
    );
    const { result, rerender } = renderHook(
      ({ loop }) => useSound(buffer, { loop }),
      { initialProps: { loop: true }, wrapper }
    );
    const values: number[] = [];
    result.current.progress.subscribe((value) => values.push(value));

    act(() => {
      result.current.play();
    });
    // The pad switches mode while the looping voice keeps playing.
    rerender({ loop: false });
    clock.now = 1.5;
    advance(20);
    expect(values.at(-1)).toBeCloseTo(0.5);
  });
});
