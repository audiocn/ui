import { runInNewContext } from "node:vm";

import { describe, expect, it } from "vitest";

import { dbToGain } from "@/lib/audio/decibels";
import {
  NEEDLE_REST,
  needleConstants,
  restingNeedle,
  stepNeedle,
  VU_BALLISTICS,
  VU_SCALE,
} from "@/lib/audio/needle";
import { vuTaper } from "@/lib/audio/taper";
import { VU_PROCESSOR_SOURCE } from "@/lib/audio/vu-worklet";

const SAMPLE_RATE = 48_000;
const BLOCK = 128;
const ZERO_VU = vuTaper(-10, 3).toPosition(0);

type Params = Record<string, Float32Array>;

interface Processor {
  port: { onmessage: ((event: { data: unknown }) => void) | null };
  process: (
    inputs: Float32Array[][],
    outputs: Float32Array[][],
    parameters: Params
  ) => boolean;
}

interface Descriptor {
  name: string;
  defaultValue: number;
}

/** One block of a 1 kHz sine at an RMS level in dBFS, or silence for null. */
const tone = (rmsDb: number | null, start: number) => {
  const block = new Float32Array(BLOCK);
  const peak = rmsDb === null ? 0 : dbToGain(rmsDb) * Math.SQRT2;
  for (let i = 0; i < BLOCK; i += 1) {
    block[i] =
      peak * Math.sin((2 * Math.PI * 1000 * (start + i)) / SAMPLE_RATE);
  }
  return block;
};

const DEFAULT_PARAMS = {
  lowPassCutoffHz: VU_BALLISTICS.lowPassCutoffHz,
  lowPassQ: VU_BALLISTICS.lowPassQ,
  maxDb: VU_SCALE.maxDb,
  minDb: VU_SCALE.minDb,
  rebound: VU_BALLISTICS.rebound,
  referenceDb: VU_SCALE.referenceDb,
};

const load = (processorOptions: {
  channel: number | "max";
  intervalMs: number;
}) => {
  const posted: number[] = [];
  let ProcessorClass:
    | ((new (options: unknown) => Processor) & {
        parameterDescriptors: Descriptor[];
      })
    | undefined;
  runInNewContext(VU_PROCESSOR_SOURCE, {
    AudioWorkletProcessor: class {
      port = {
        onmessage: null,
        postMessage: (position: number) => posted.push(position),
      };
    },
    registerProcessor: (_name: string, constructor: typeof ProcessorClass) => {
      ProcessorClass = constructor;
    },
    sampleRate: SAMPLE_RATE,
  });
  if (!ProcessorClass) {
    throw new Error("The processor was not registered.");
  }
  const descriptors = ProcessorClass.parameterDescriptors;
  const processor = new ProcessorClass({ processorOptions });
  let params: Params = Object.fromEntries(
    Object.entries(DEFAULT_PARAMS).map(([name, value]) => [
      name,
      new Float32Array([value]),
    ])
  );
  let sample = 0;
  /** A 1 kHz sine per channel, at an RMS level in dBFS, or silence for null. */
  const run = (ms: number, levels: (number | null)[]) => {
    const blocks = Math.round((ms * SAMPLE_RATE) / 1000 / BLOCK);
    for (let block = 0; block < blocks; block += 1) {
      const start = sample;
      const input = levels.map((rmsDb) => tone(rmsDb, start));
      sample += BLOCK;
      processor.process([input], [[new Float32Array(BLOCK)]], params);
    }
  };
  const setParams = (next: Partial<typeof DEFAULT_PARAMS>) => {
    params = {
      ...params,
      ...Object.fromEntries(
        Object.entries(next).map(([name, value]) => [
          name,
          new Float32Array([value]),
        ])
      ),
    };
  };
  const send = (data: unknown) => processor.port.onmessage?.({ data });
  return { descriptors, posted, processor, run, send, setParams };
};

describe("VU_PROCESSOR_SOURCE", () => {
  it("declares a k-rate param for each needle and scale setting", () => {
    const { descriptors } = load({ channel: "max", intervalMs: 1 });
    const defaults = Object.fromEntries(
      descriptors.map((d) => [d.name, d.defaultValue])
    );
    expect(defaults).toEqual(DEFAULT_PARAMS);
  });

  it("moves exactly as stepNeedle does", () => {
    const { posted, run } = load({ channel: "max", intervalMs: 1 });
    run(500, [-18]);
    const constants = needleConstants(VU_BALLISTICS, {
      maxDb: 3,
      minDb: -10,
      referenceDb: -18,
    });
    const state = restingNeedle();
    const expected: number[] = [];
    for (let i = 0; i < posted.length * 48; i += 1) {
      const x =
        dbToGain(-18) *
        Math.SQRT2 *
        Math.sin((2 * Math.PI * 1000 * i) / SAMPLE_RATE);
      stepNeedle(state, Math.abs(x), constants, 1 / SAMPLE_RATE);
      if ((i + 1) % 48 === 0) {
        expected.push(state.position);
      }
    }
    expect(posted).toHaveLength(expected.length);
    for (const [index, position] of posted.entries()) {
      expect(position).toBeCloseTo(expected[index] ?? Number.NaN, 4);
    }
  });

  it("posts the position every intervalMs", () => {
    const { posted, run } = load({ channel: "max", intervalMs: 10 });
    run(1000, [-18]);
    expect(posted.length).toBeGreaterThanOrEqual(99);
    expect(posted.length).toBeLessThanOrEqual(101);
  });

  it("follows the louder channel, or the one it is given", () => {
    const max = load({ channel: "max", intervalMs: 1 });
    const left = load({ channel: 0, intervalMs: 1 });
    const right = load({ channel: 1, intervalMs: 1 });
    for (const meter of [max, left, right]) {
      meter.run(3000, [null, -18]);
    }
    expect(max.posted.at(-1)).toBeCloseTo(ZERO_VU, 2);
    expect(left.posted.at(-1)).toBe(NEEDLE_REST);
    expect(right.posted.at(-1)).toBeCloseTo(ZERO_VU, 2);
  });

  it("reads a mono input on either channel", () => {
    const right = load({ channel: 1, intervalMs: 1 });
    right.run(3000, [-18]);
    expect(right.posted.at(-1)).toBeCloseTo(ZERO_VU, 2);
  });

  it("follows param changes without losing the needle's place", () => {
    const { posted, run, setParams } = load({ channel: "max", intervalMs: 1 });
    run(3000, [-18]);
    expect(posted.at(-1)).toBeCloseTo(ZERO_VU, 2);
    setParams({ referenceDb: -20 });
    run(3000, [-18]);
    expect(posted.at(-1)).toBeCloseTo(vuTaper(-10, 3).toPosition(2), 2);
  });

  it("goes back to rest on reset", () => {
    const { posted, run, send } = load({ channel: "max", intervalMs: 1 });
    run(1000, [-18]);
    send("reset");
    run(3, [null]);
    expect(posted.at(-1)).toBe(NEEDLE_REST);
  });

  it("asks to be released after stop", () => {
    const { processor, send } = load({ channel: "max", intervalMs: 1 });
    send("stop");
    expect(processor.process([[]], [[new Float32Array(BLOCK)]], {})).toBe(
      false
    );
  });

  it("falls to rest with nothing connected", () => {
    const { posted, run } = load({ channel: "max", intervalMs: 1 });
    run(1000, []);
    expect(posted.at(-1)).toBe(NEEDLE_REST);
  });
});
