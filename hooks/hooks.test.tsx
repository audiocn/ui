import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useAudioDevices } from "@/hooks/use-audio-devices";
import { useClipHold } from "@/hooks/use-clip-hold";
import { createDemoSignal } from "@/hooks/use-demo-signal";
import { useLevel } from "@/hooks/use-level";
import { useMicrophone } from "@/hooks/use-microphone";
import { isChannelAudible, mixerReducer, useMixer } from "@/hooks/use-mixer";
import type { MixerState } from "@/hooks/use-mixer";
import { createFrameEmitter, createFrameRelay } from "@/lib/audio/frame-source";
import type { MeterFrame } from "@/lib/audio/types";
import { advance, useFakeFrames } from "@/test/fake-frames";

const baseState: MixerState = {
  channels: [
    { gainDb: 0, id: "mic", monitor: false, muted: false, pan: 0, solo: false },
    {
      gainDb: -6,
      id: "music",
      monitor: true,
      muted: false,
      pan: 0,
      solo: false,
    },
  ],
  master: { gainDb: 0, muted: false },
};

describe("mixerReducer", () => {
  it("patches one channel and clamps pan", () => {
    const next = mixerReducer(baseState, {
      id: "mic",
      patch: { pan: 3 },
      type: "channel",
    });
    expect(next.channels[0]?.pan).toBe(1);
    expect(next.channels[1]).toBe(baseState.channels[1]);
  });

  it("solos exclusively when asked", () => {
    const soloed = mixerReducer(baseState, {
      exclusive: false,
      id: "mic",
      solo: true,
      type: "solo",
    });
    const exclusive = mixerReducer(soloed, {
      exclusive: true,
      id: "music",
      solo: true,
      type: "solo",
    });
    expect(exclusive.channels.map((channel) => channel.solo)).toEqual([
      false,
      true,
    ]);
  });

  it("adds and removes channels", () => {
    const added = mixerReducer(baseState, {
      channel: { id: "system" },
      type: "add",
    });
    expect(added.channels.at(-1)).toMatchObject({
      gainDb: 0,
      id: "system",
      muted: false,
    });
    expect(
      mixerReducer(added, { channel: { id: "system" }, type: "add" })
    ).toBe(added);
    expect(
      mixerReducer(added, { id: "mic", type: "remove" }).channels
    ).toHaveLength(2);
  });
});

describe("isChannelAudible", () => {
  it("silences muted channels and channels outside a solo", () => {
    const muted = mixerReducer(baseState, {
      id: "mic",
      patch: { muted: true },
      type: "channel",
    });
    expect(isChannelAudible(muted, "mic")).toBe(false);
    const soloed = mixerReducer(baseState, {
      exclusive: false,
      id: "music",
      solo: true,
      type: "solo",
    });
    expect(isChannelAudible(soloed, "mic")).toBe(false);
    expect(isChannelAudible(soloed, "music")).toBe(true);
  });
});

describe("useMixer", () => {
  it("manages state and reports dimmed channels", () => {
    const { result } = renderHook(() =>
      useMixer({ channels: [{ id: "a" }, { id: "b" }] })
    );
    act(() => {
      result.current.setGain("a", -12);
      result.current.setSolo("b", true);
    });
    expect(result.current.channel("a")?.gainDb).toBe(-12);
    expect(result.current.isDimmed("a")).toBe(true);
    expect(result.current.isAudible("b")).toBe(true);
    act(() => {
      result.current.reset();
    });
    expect(result.current.channel("a")?.gainDb).toBe(0);
  });

  it("works controlled", () => {
    const onStateChange = vi.fn();
    const { result } = renderHook(() =>
      useMixer({ onStateChange, state: baseState })
    );
    act(() => {
      result.current.setMuted("mic", true);
    });
    expect(onStateChange).toHaveBeenCalledWith(
      expect.objectContaining({
        channels: expect.arrayContaining([
          expect.objectContaining({ id: "mic", muted: true }),
        ]),
      })
    );
    expect(result.current.channel("mic")?.muted).toBe(false);
  });

  it("persists to localStorage", () => {
    const { result } = renderHook(() =>
      useMixer({ channels: [{ id: "a" }], persistKey: "test-mixer" })
    );
    act(() => {
      result.current.setGain("a", -3);
    });
    const saved = JSON.parse(
      window.localStorage.getItem("test-mixer") ?? "{}"
    ) as MixerState;
    expect(saved.channels[0]?.gainDb).toBe(-3);
    window.localStorage.removeItem("test-mixer");
  });
});

describe("frame sources over time", () => {
  beforeEach(() => {
    useFakeFrames();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("useLevel samples a source at its interval", () => {
    const emitter = createFrameEmitter<MeterFrame>();
    const { result } = renderHook(() => useLevel(emitter, { intervalMs: 100 }));
    emitter.emit({ channels: [{ peakDb: -6, rmsDb: -12 }] });
    advance(120);
    expect(result.current).toEqual({ peakDb: -6, rmsDb: -12, zone: "clip" });
  });

  it("useClipHold counts separate clips and releases after the hold", () => {
    const { result } = renderHook(() => useClipHold({ holdMs: 200 }));
    act(() => {
      result.current.report(0);
      result.current.report(0);
      result.current.report(-20);
      result.current.report(-0.5);
    });
    expect(result.current.count).toBe(2);
    expect(result.current.clipping).toBe(true);
    advance(250);
    expect(result.current.clipping).toBe(false);
  });

  it("createDemoSignal emits frames only while subscribed", () => {
    const signal = createDemoSignal({ channels: 2, kind: "tone" });
    const listener = vi.fn();
    const unsubscribe = signal.meter.subscribe(listener);
    advance(100);
    expect(listener).toHaveBeenCalled();
    const frame = listener.mock.calls.at(-1)?.[0] as MeterFrame;
    expect(frame.channels).toHaveLength(2);
    expect(frame.channels[0]?.peakDb).toBeCloseTo(-12, 0);
    unsubscribe();
    listener.mockClear();
    advance(100);
    expect(listener).not.toHaveBeenCalled();
  });

  it("createFrameRelay keeps subscribers across source changes", () => {
    const relay = createFrameRelay<number>();
    const first = createFrameEmitter<number>();
    const second = createFrameEmitter<number>();
    const listener = vi.fn();
    relay.subscribe(listener);
    relay.setSource(first);
    first.emit(1);
    relay.setSource(second);
    first.emit(2);
    second.emit(3);
    expect(listener.mock.calls.map(([value]) => value)).toEqual([1, 3]);
  });
});

describe("browser device hooks", () => {
  const track = { addEventListener: vi.fn(), stop: vi.fn() };
  const stream = {
    getAudioTracks: () => [track],
    getTracks: () => [track],
  } as unknown as MediaStream;

  beforeEach(() => {
    Object.defineProperty(navigator, "mediaDevices", {
      configurable: true,
      value: {
        addEventListener: vi.fn(),
        enumerateDevices: vi.fn(async () => [
          {
            deviceId: "default",
            groupId: "g",
            kind: "audioinput",
            label: "Default - Built-in",
          },
          {
            deviceId: "usb",
            groupId: "u",
            kind: "audioinput",
            label: "USB mic",
          },
          {
            deviceId: "cam",
            groupId: "c",
            kind: "videoinput",
            label: "Camera",
          },
        ]),
        getUserMedia: vi.fn(async () => stream),
        removeEventListener: vi.fn(),
      },
    });
  });

  it("useAudioDevices lists audio inputs and marks the default", async () => {
    const { result } = renderHook(() => useAudioDevices());
    await waitFor(() => {
      expect(result.current.devices).toHaveLength(2);
    });
    expect(result.current.devices[0]).toMatchObject({
      id: "default",
      isDefault: true,
    });
    expect(result.current.permission).toBe("granted");
  });

  it("useMicrophone opens the device with processing off and stops it", async () => {
    const { result, unmount } = renderHook(() =>
      useMicrophone({ deviceId: "usb", enabled: true })
    );
    await waitFor(() => {
      expect(result.current.status).toBe("active");
    });
    expect(navigator.mediaDevices.getUserMedia).toHaveBeenCalledWith({
      audio: {
        autoGainControl: false,
        deviceId: { exact: "usb" },
        echoCancellation: false,
        noiseSuppression: false,
      },
    });
    unmount();
    expect(track.stop).toHaveBeenCalled();
  });
});
