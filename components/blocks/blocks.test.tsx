import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { useState } from "react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { MicSetup } from "@/components/blocks/mic-setup/mic-setup";
import { MusicPlayer } from "@/components/blocks/music-player/music-player";
import { Soundboard } from "@/components/blocks/soundboard/soundboard";
import { SystemAudioSettings } from "@/components/blocks/system-audio-settings/system-audio-settings";
import { AudioContextProvider } from "@/hooks/use-audio-context";
import { createFrameEmitter } from "@/lib/audio/frame-source";
import type { MeterFrame } from "@/lib/audio/types";
import { createFakeAudioContext } from "@/test/fake-audio";

const TRACKS = [{ id: "demo", src: "demo.wav", title: "Demo" }];

describe("blocks rendered twice on one page", () => {
  it("keep every DOM id unique", () => {
    render(
      <>
        <MicSetup />
        <MicSetup />
        <MusicPlayer defaultTracks={TRACKS} duckingSource={null} />
        <MusicPlayer defaultTracks={TRACKS} duckingSource={null} />
        <SystemAudioSettings />
        <SystemAudioSettings />
      </>
    );
    const ids = [...document.querySelectorAll("[id]")].map(({ id }) => id);
    expect(ids.length).toBeGreaterThan(0);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("toggle the switch of the instance whose label was clicked", () => {
    render(
      <>
        <MicSetup />
        <MicSetup />
      </>
    );
    const [, secondLabel] = screen.getAllByText("Mute microphone");
    const [first, second] = screen.getAllByRole("switch", {
      name: "Mute microphone",
    });
    fireEvent.click(secondLabel as HTMLElement);
    expect(second).toHaveAttribute("aria-checked", "true");
    expect(first).toHaveAttribute("aria-checked", "false");
  });
});

const setDisplayMedia = (getDisplayMedia: () => Promise<MediaStream>) => {
  Object.defineProperty(navigator, "mediaDevices", {
    configurable: true,
    value: {
      addEventListener: vi.fn(),
      getDisplayMedia,
      removeEventListener: vi.fn(),
    },
  });
};

const audioStream = () => {
  const track = { addEventListener: vi.fn(), stop: vi.fn() };
  return {
    getAudioTracks: () => [track],
    getTracks: () => [track],
    getVideoTracks: () => [],
  } as unknown as MediaStream;
};

describe("blocks driving Web Audio", () => {
  beforeEach(() => {
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
    vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => {
      // jsdom has no media pipeline.
    });
    vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => {
      // jsdom has no media pipeline.
    });
    vi.spyOn(HTMLMediaElement.prototype, "play").mockResolvedValue();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("system audio settings: the switch restarts a capture that ended while enabled stayed true", async () => {
    const getDisplayMedia = vi.fn(() =>
      Promise.reject(new DOMException("Cancelled", "NotAllowedError"))
    );
    setDisplayMedia(getDisplayMedia);
    render(<SystemAudioSettings enabled onEnabledChange={vi.fn()} />);
    await waitFor(() => {
      expect(getDisplayMedia).toHaveBeenCalledTimes(1);
    });

    fireEvent.click(
      await screen.findByRole("switch", { name: /capture system audio/iu })
    );
    await waitFor(() => {
      expect(getDisplayMedia).toHaveBeenCalledTimes(2);
    });
  });

  it("system audio settings: parent re-renders don't rebuild the live source", async () => {
    const { context, fake } = createFakeAudioContext();
    setDisplayMedia(() => Promise.resolve(audioStream()));
    const streams: (MediaStream | null)[] = [];
    const Parent = () => {
      const [ticks, setTicks] = useState(0);
      return (
        <AudioContextProvider context={context}>
          <button onClick={() => setTicks(ticks + 1)} type="button">
            Tick
          </button>
          <SystemAudioSettings
            onStreamChange={(stream) => streams.push(stream)}
          />
        </AudioContextProvider>
      );
    };
    render(<Parent />);
    fireEvent.click(
      screen.getByRole("switch", { name: /capture system audio/iu })
    );
    await waitFor(() => {
      expect(fake.createMediaStreamSource).toHaveBeenCalledTimes(1);
    });
    const reported = streams.length;

    for (let tick = 0; tick < 3; tick += 1) {
      fireEvent.click(screen.getByRole("button", { name: "Tick" }));
    }
    expect(fake.createMediaStreamSource).toHaveBeenCalledTimes(1);
    expect(streams).toHaveLength(reported);
  });

  it("soundboard: Stop all stops every playing pad", () => {
    const { context } = createFakeAudioContext();
    const buffer = { duration: 2 } as AudioBuffer;
    const wrapper = (children: ReactNode) => (
      <AudioContextProvider context={context}>{children}</AudioContextProvider>
    );
    const { container } = render(
      wrapper(
        <Soundboard
          defaultSounds={[
            { id: "kick", label: "Kick", src: buffer },
            { id: "snare", label: "Snare", src: buffer },
          ]}
        />
      )
    );
    const pads = [...container.querySelectorAll("[data-sound-pad]")].slice(
      0,
      2
    );
    for (const pad of pads) {
      fireEvent.click(pad);
    }
    for (const pad of pads) {
      expect(pad).toHaveAttribute("data-playing");
    }

    fireEvent.click(screen.getByRole("button", { name: /stop all/iu }));
    for (const pad of pads) {
      expect(pad).not.toHaveAttribute("data-playing");
    }
  });

  it("music player: a duck schedules its own release, so silence brings the music back", () => {
    const { context, gains } = createFakeAudioContext();
    const voice = createFrameEmitter<MeterFrame>();
    render(
      <AudioContextProvider context={context}>
        <MusicPlayer defaultTracks={TRACKS} duckingSource={voice} />
      </AudioContextProvider>
    );
    act(() => {
      voice.emit({ channels: [{ peakDb: -10, rmsDb: -20 }] });
    });

    const calls = gains.flatMap((node) => node.gain.setTargetAtTime.mock.calls);
    // Ducked now, and back to unity once the hold runs out with no new frame.
    expect(calls.some(([value, at]) => value < 1 && at === 0)).toBe(true);
    expect(calls.some(([value, at]) => value === 1 && at > 0)).toBe(true);
  });
});
