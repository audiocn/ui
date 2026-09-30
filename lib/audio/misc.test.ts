import { describe, expect, it, vi } from "vitest";

import { createFrameEmitter } from "@/lib/audio/frame-source";
import { formatTime } from "@/lib/audio/time";
import { DEFAULT_ZONES, zoneForDb } from "@/lib/audio/zones";

describe("zoneForDb", () => {
  it("uses the default thresholds", () => {
    expect(zoneForDb(-30)).toBe("ok");
    expect(zoneForDb(-20)).toBe("warn");
    expect(zoneForDb(-9)).toBe("clip");
    expect(zoneForDb(Number.NEGATIVE_INFINITY)).toBe("ok");
  });

  it("accepts custom zones", () => {
    const zones = [
      ...DEFAULT_ZONES.slice(0, 1),
      { fromDb: -3, zone: "clip" as const },
    ];
    expect(zoneForDb(-9, zones)).toBe("ok");
    expect(zoneForDb(-2, zones)).toBe("clip");
  });
});

describe("formatTime", () => {
  it("formats minutes and seconds", () => {
    expect(formatTime(0)).toBe("0:00");
    expect(formatTime(84.9)).toBe("1:24");
  });

  it("switches to hours from one hour", () => {
    expect(formatTime(3725)).toBe("1:02:05");
    expect(formatTime(65, { hours: true })).toBe("0:01:05");
  });

  it("prefixes remaining time and guards bad input", () => {
    expect(formatTime(30, { remaining: true })).toBe("−0:30");
    expect(formatTime(Number.NaN)).toBe("0:00");
    expect(formatTime(-5)).toBe("0:00");
  });
});

describe("createFrameEmitter", () => {
  it("delivers frames to subscribers until they unsubscribe", () => {
    const emitter = createFrameEmitter<number>();
    const listener = vi.fn();
    const unsubscribe = emitter.subscribe(listener);
    emitter.emit(1);
    unsubscribe();
    emitter.emit(2);
    expect(listener).toHaveBeenCalledTimes(1);
    expect(listener).toHaveBeenCalledWith(1);
    expect(emitter.latest).toBe(2);
  });
});
