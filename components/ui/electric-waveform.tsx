"use client";

import { useCallback, useEffect, useImperativeHandle, useRef } from "react";
import type { ComponentProps, Ref } from "react";

import { useFrameSource } from "@/hooks/use-frame-source";
import { useReducedMotion } from "@/hooks/use-reduced-motion";
import { createBarLevels } from "@/lib/audio/bar-levels";
import { clamp } from "@/lib/audio/decibels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import type { FrameSource, VisualFrame } from "@/lib/audio/types";
import {
  ELECTRIC_CANVAS_CLASS,
  ELECTRIC_GLOW_CLASS,
  clearElectricCanvas,
  createElectricSparks,
  createRandom,
  displace,
  emitSpark,
  fitElectricCanvases,
  moveSparks,
  readElectricColors,
  strokeSparks,
} from "@/lib/electric";
import type {
  ElectricCanvasSize,
  ElectricColors,
  ElectricSparks,
} from "@/lib/electric";
import { cn } from "@/lib/utils";

const DEFAULT_INTENSITY = 0.6;
const DEFAULT_LINE_WIDTH = 3;
const REDUCED_MOTION_INTERVAL_MS = 250;
const COLOR_REFRESH_FRAMES = 30;
const MS_PER_SECOND = 1000;
const FRAME_MS = 16.67;
const MAX_STEP_SECONDS = 0.05;
const TWO_PI = Math.PI * 2;
/** Below this the line counts as silent. */
const SIGNAL_THRESHOLD = 0.02;
/** Width of the faded ends, in pixels. */
const FADE_PX = 24;

/** One point every this many pixels along the line. */
const POINT_SPACING_PX = 4;
/** Crackle is built in short chunks of 2^2 segments that share their ends. */
const CHUNK = 2 ** 2;
const MIN_POINTS = CHUNK * 8 + 1;
const MAX_POINTS = CHUNK * 128 + 1;
/** How much each finer split of the crackle bends compared with the one before. */
const ROUGHNESS = 0.8;
/** Each jagged shape holds at least this long, so the crackle never strobes. */
const JITTER_MS = 42;
/** Brightness flicker, at most 12%. */
const FLICKER = 0.12;
/** Share of the distance to the new shape still left after one frame. */
const EASE_PER_FRAME = 0.45;
/** Loudness left after one frame of silence. */
const LOUDNESS_RELEASE = 0.92;
/** Crackle at rest, in pixels at full intensity. */
const HUM_PX = 1.5;
/** Extra crackle at full level, in pixels at full intensity. */
const CRACKLE_PX = 6;

/** Cycles across the width, drift in radians per second and weight of each band group, lows first. */
const WAVE_PARTIALS = [
  { cycles: 1.5, drift: 0.8, weight: 0.6 },
  { cycles: 2.5, drift: -1.3, weight: 0.4 },
  { cycles: 4, drift: 2.1, weight: 0.25 },
  { cycles: 6.5, drift: -3.2, weight: 0.16 },
] as const;

/** Scope mode lifts quiet signals by at most this much. */
const SCOPE_MAX_GAIN = 4;
/** Scope mode scales its running peak to this height. */
const SCOPE_TARGET = 0.9;
/** Share of the scope's running peak left after one frame. */
const SCOPE_PEAK_RELEASE = 0.985;

/** Widths per second the loading pulse travels. */
const LOADING_SPEED = 0.55;
/** Where the loading pulse rests with reduced motion: the middle. */
const LOADING_REST_SECONDS = 0.7 / LOADING_SPEED;

const MAX_BRANCHES = 3;
const BRANCH_POINTS = 2 ** 3 + 1;
/** A point must be this far from the middle to throw a fork. */
const BRANCH_THRESHOLD = 0.3;
const BRANCH_CHANCE = 0.6;
const BRANCH_CANDIDATES = 3;
const BRANCH_MIN_PX = 10;
const BRANCH_MAX_PX = 30;
const BRANCH_MIN_MS = 60;
const BRANCH_MAX_MS = 140;
/** How far a fork bends, relative to its length. */
const BRANCH_BOW = 0.35;

const MAX_SPARKS = 48;
/** A rise in loudness this big in one frame throws sparks. */
const SPARK_RISE = 0.1;
const SPARK_MIN_LEVEL = 0.3;
const SPARK_COOLDOWN_MS = 90;
/** Pixels per second squared. */
const SPARK_GRAVITY = 520;
/** Pixels per second. */
const SPARK_SPEED = 170;
/** Pixels per second, sideways. */
const SPARK_SPREAD = 90;
const SPARK_MIN_MS = 260;
const SPARK_MAX_MS = 560;
const SPARK_TRAIL_SECONDS = 0.03;

export type ElectricWaveformMode = "wave" | "scope";

export interface ElectricWaveformActions {
  /** Paint a frame directly. */
  paint: (frame: VisualFrame) => void;
  /** Forget the last frame, so the line falls flat. */
  clear: () => void;
}

export interface ElectricWaveformProps extends ComponentProps<"div"> {
  source?: FrameSource<VisualFrame> | null;
  /**
   * `wave` draws a smooth wave shaped by the frequency bands. `scope` draws
   * the signal itself, like an oscilloscope. Default `wave`.
   */
  mode?: ElectricWaveformMode;
  /** Runs a pulse along the line, for connecting or thinking states. Default false. */
  loading?: boolean;
  /** How jagged and restless the line is, 0..1. Default 0.6. */
  intensity?: number;
  /** Forks of lightning branch off the peaks. Default true. */
  arcs?: boolean;
  /** Sparks fly off the peaks on sudden rises. Default true. */
  sparks?: boolean;
  /** Visual gain. Default 1. */
  sensitivity?: number;
  /** Line width in pixels. Default 3. */
  lineWidth?: number;
  /** Fade the left and right ends. Default true. */
  fadeEdges?: boolean;
  actionsRef?: Ref<ElectricWaveformActions>;
}

/** A fork of lightning off the line. */
export interface ElectricBranch {
  /** The point on the line it leaves from. */
  point: number;
  /** 1 up, −1 down: away from the middle. */
  direction: number;
  /** Pixels. */
  length: number;
  /** Sideways drift of the far end, relative to the length. */
  lean: number;
  bornMs: number;
  /** 0 when the fork is gone. */
  lifeMs: number;
  /** Bend at each point, in units of the fork's bow. */
  jitter: Float32Array;
}

export interface ElectricTraceGeometry {
  /** CSS pixels. */
  width: number;
  height: number;
  lineWidth: number;
}

export interface ElectricTraceOptions {
  mode: ElectricWaveformMode;
  loading: boolean;
  intensity: number;
  arcs: boolean;
  sparks: boolean;
  sensitivity: number;
  /** A still line: no crackle, forks, sparks or drift. */
  reducedMotion: boolean;
  /** Makes the crackle repeatable. Each trace gets its own by default. */
  seed?: number;
}

export interface ElectricTrace {
  /** Points in use along the width. */
  count: number;
  /** Height of the line at each point, −1..1, up positive. */
  heights: Float32Array;
  /** Crackle at each point, in units of its amplitude. */
  crackle: Float32Array;
  /** How loud the line is, 0..1. */
  loudness: number;
  /** Brightness, 0.88..1. */
  flicker: number;
  branches: ElectricBranch[];
  sparks: ElectricSparks;
  /** Advances to `nowMs` with the latest frame. Returns true when there is a signal. */
  step: (
    nowMs: number,
    frame: VisualFrame | null,
    geometry: ElectricTraceGeometry
  ) => boolean;
}

/**
 * Where the signal first rises through zero in its first quarter, between
 * samples, so a steady tone stands still. 0 when it never does.
 */
export const triggerIndex = (samples: ArrayLike<number>): number => {
  const limit = Math.floor(samples.length / 4);
  for (let index = 1; index <= limit; index += 1) {
    const before = samples[index - 1] ?? 0;
    const after = samples[index] ?? 0;
    if (before < 0 && after >= 0) {
      return index - 1 + -before / (after - before);
    }
  }
  return 0;
};

/** Fits three quarters of the samples, from the trigger, into `count` heights. */
const fillScope = (
  samples: ArrayLike<number>,
  out: Float32Array,
  count: number,
  gain: number
) => {
  const start = triggerIndex(samples);
  const span = Math.floor(samples.length * 0.75) - 1;
  for (let point = 0; point < count; point += 1) {
    const position = start + (span * point) / (count - 1);
    const index = Math.floor(position);
    const before = samples[index] ?? 0;
    const after = samples[Math.min(samples.length - 1, index + 1)] ?? before;
    const value = before + (after - before) * (position - index);
    out[point] = clamp(value * gain, -1, 1);
  }
};

/** A short wave packet that travels left to right. */
const loadingPulse = (position: number, seconds: number) => {
  const center = ((seconds * LOADING_SPEED) % 1.4) - 0.2;
  const envelope = Math.exp(-((position - center) ** 2) / 0.012);
  return 0.6 * envelope * Math.sin(TWO_PI * (position * 7 - seconds * 2.5));
};

/** Points for a width: one every few pixels, in whole crackle chunks. */
const pointCountFor = (width: number) =>
  clamp(
    Math.round(width / POINT_SPACING_PX / CHUNK) * CHUNK + 1,
    MIN_POINTS,
    MAX_POINTS
  );

const middleOf = (geometry: ElectricTraceGeometry) => geometry.height / 2;

/** Pixels from the middle to a height of 1, leaving room for the line. */
const reachOf = (geometry: ElectricTraceGeometry) =>
  Math.max(0, geometry.height / 2 - Math.max(geometry.lineWidth * 2, 6));

const xOf = (geometry: ElectricTraceGeometry, count: number, point: number) =>
  (point / (count - 1)) * geometry.width;

/** Crackle in pixels at a point: louder lines and higher peaks crackle more. */
const crackleAt = (trace: ElectricTrace, intensity: number, point: number) =>
  intensity *
  (HUM_PX +
    CRACKLE_PX *
      (0.4 * trace.loudness + 0.6 * Math.abs(trace.heights[point] ?? 0)));

/** Where a point of the line is drawn, in pixels down from the top. */
const yOf = (
  trace: ElectricTrace,
  geometry: ElectricTraceGeometry,
  intensity: number,
  point: number,
  scale = 1,
  crackleShare = 1
) =>
  middleOf(geometry) -
  (trace.heights[point] ?? 0) * scale * reachOf(geometry) -
  (trace.crackle[point] ?? 0) *
    crackleAt(trace, intensity, point) *
    crackleShare;

const peakOf = (values: ArrayLike<number>, count: number) => {
  let peak = 0;
  for (let index = 0; index < count; index += 1) {
    peak = Math.max(peak, Math.abs(values[index] ?? 0));
  }
  return peak;
};

let traceCount = 0;

/**
 * The moving parts of the electric line, apart from any canvas: its shape,
 * crackle, forks and sparks. With reduced motion it is a still, smooth line.
 */
export const createElectricTrace = ({
  arcs,
  intensity,
  loading,
  mode,
  reducedMotion,
  seed,
  sensitivity,
  sparks,
}: ElectricTraceOptions): ElectricTrace => {
  traceCount += 1;
  const random = createRandom(seed ?? traceCount);
  const signed = () => random() * 2 - 1;
  const targets = new Float32Array(MAX_POINTS);
  const bands = createBarLevels({
    barCount: WAVE_PARTIALS.length,
    idle: "static",
    loading: false,
    minLevel: 0,
    mirrored: false,
    reducedMotion,
  });
  const phases = new Float32Array(WAVE_PARTIALS.length);
  let lastMs = 0;
  let lastJitterMs = -Infinity;
  let lastSparkMs = -Infinity;
  let lastStepSeconds = 0;
  let scopePeak = 0;
  let signalPeak = 0;
  let previousSignalPeak = 0;
  let primed = false;

  const trace: ElectricTrace = {
    branches: Array.from({ length: MAX_BRANCHES }, () => ({
      bornMs: 0,
      direction: 1,
      jitter: new Float32Array(BRANCH_POINTS),
      lean: 0,
      length: 0,
      lifeMs: 0,
      point: 0,
    })),
    count: MIN_POINTS,
    crackle: new Float32Array(MAX_POINTS),
    flicker: 1,
    heights: new Float32Array(MAX_POINTS),
    loudness: 0,
    sparks: createElectricSparks(MAX_SPARKS),
    step: () => false,
  };

  const fillWave = (frame: VisualFrame | null, nowMs: number) => {
    bands.step(nowMs, frame?.bands ?? null);
    for (let point = 0; point < trace.count; point += 1) {
      const position = point / (trace.count - 1);
      let height = 0;
      for (const [index, partial] of WAVE_PARTIALS.entries()) {
        height +=
          partial.weight *
          (bands.levels[index] ?? 0) *
          Math.sin(TWO_PI * partial.cycles * position + (phases[index] ?? 0));
      }
      targets[point] = clamp(height * sensitivity, -1, 1);
    }
  };

  const fillTargets = (nowMs: number, frame: VisualFrame | null) => {
    const { count } = trace;
    if (loading) {
      const seconds = reducedMotion
        ? LOADING_REST_SECONDS
        : nowMs / MS_PER_SECOND;
      for (let point = 0; point < count; point += 1) {
        targets[point] = loadingPulse(point / (count - 1), seconds);
      }
      return;
    }
    const samples = frame?.timeDomain;
    if (mode === "scope" && samples && samples.length > 1) {
      scopePeak = Math.max(
        peakOf(samples, samples.length),
        scopePeak * SCOPE_PEAK_RELEASE
      );
      const gain =
        sensitivity * Math.min(SCOPE_MAX_GAIN, SCOPE_TARGET / (scopePeak || 1));
      fillScope(samples, targets, count, gain);
      return;
    }
    fillWave(frame, nowMs);
  };

  const rerollCrackle = () => {
    trace.crackle[0] = signed() * 0.5;
    for (let start = 0; start + CHUNK < trace.count; start += CHUNK) {
      displace(
        trace.crackle,
        start,
        CHUNK + 1,
        [trace.crackle[start] ?? 0, signed() * 0.5],
        random,
        ROUGHNESS
      );
    }
    trace.flicker = 1 - FLICKER * random();
  };

  /** Picks the highest of a few random points, so forks favour the peaks. */
  const pickPeak = () => {
    let best = Math.floor(random() * trace.count);
    for (let candidate = 1; candidate < BRANCH_CANDIDATES; candidate += 1) {
      const point = Math.floor(random() * trace.count);
      if (
        Math.abs(trace.heights[point] ?? 0) > Math.abs(trace.heights[best] ?? 0)
      ) {
        best = point;
      }
    }
    return best;
  };

  const updateBranches = (nowMs: number) => {
    const point = pickPeak();
    const height = trace.heights[point] ?? 0;
    const charged =
      Math.abs(height) >= BRANCH_THRESHOLD &&
      random() < BRANCH_CHANCE * intensity * Math.abs(height);
    const free = trace.branches.find((branch) => branch.lifeMs === 0);
    if (charged && free) {
      free.point = point;
      free.direction = height >= 0 ? 1 : -1;
      free.length =
        (BRANCH_MIN_PX + random() * (BRANCH_MAX_PX - BRANCH_MIN_PX)) *
        (0.6 + 0.8 * intensity);
      free.lean = signed() * 0.6;
      free.bornMs = nowMs;
      free.lifeMs = BRANCH_MIN_MS + random() * (BRANCH_MAX_MS - BRANCH_MIN_MS);
    }
    for (const branch of trace.branches) {
      if (branch.lifeMs > 0) {
        displace(
          branch.jitter,
          0,
          BRANCH_POINTS,
          [0, signed() * 0.5],
          random,
          ROUGHNESS
        );
      }
    }
  };

  const expireBranches = (nowMs: number) => {
    for (const branch of trace.branches) {
      if (branch.lifeMs > 0 && nowMs - branch.bornMs >= branch.lifeMs) {
        branch.lifeMs = 0;
      }
    }
  };

  const throwSparks = (nowMs: number, geometry: ElectricTraceGeometry) => {
    const rise = signalPeak - previousSignalPeak;
    const rested = nowMs - lastSparkMs >= SPARK_COOLDOWN_MS;
    if (rise < SPARK_RISE || signalPeak < SPARK_MIN_LEVEL || !rested) {
      return;
    }
    lastSparkMs = nowMs;
    let peak = 0;
    for (let point = 1; point < trace.count; point += 1) {
      if (
        Math.abs(trace.heights[point] ?? 0) > Math.abs(trace.heights[peak] ?? 0)
      ) {
        peak = point;
      }
    }
    const upward = (trace.heights[peak] ?? 0) >= 0 ? -1 : 1;
    const x = xOf(geometry, trace.count, peak);
    const y = yOf(trace, geometry, intensity, peak);
    const count = 3 + Math.floor(random() * 3);
    for (let spark = 0; spark < count; spark += 1) {
      emitSpark(trace.sparks, {
        lifeMs: SPARK_MIN_MS + random() * (SPARK_MAX_MS - SPARK_MIN_MS),
        vx: signed() * SPARK_SPREAD,
        vy: upward * SPARK_SPEED * (0.5 + random()) * (0.5 + intensity),
        x,
        y,
      });
    }
  };

  const ease = (elapsedMs: number) => {
    const share = reducedMotion
      ? 1
      : 1 - EASE_PER_FRAME ** (elapsedMs / FRAME_MS);
    for (let point = 0; point < trace.count; point += 1) {
      const height = trace.heights[point] ?? 0;
      trace.heights[point] = height + ((targets[point] ?? 0) - height) * share;
    }
    const release = LOUDNESS_RELEASE ** (elapsedMs / FRAME_MS);
    trace.loudness = Math.max(
      peakOf(trace.heights, trace.count),
      trace.loudness * release
    );
  };

  const animate = (nowMs: number, geometry: ElectricTraceGeometry) => {
    if (nowMs - lastJitterMs >= JITTER_MS) {
      lastJitterMs = nowMs;
      rerollCrackle();
      if (arcs) {
        updateBranches(nowMs);
      }
    }
    expireBranches(nowMs);
    if (sparks) {
      if (primed) {
        throwSparks(nowMs, geometry);
      }
      moveSparks(trace.sparks, lastStepSeconds, SPARK_GRAVITY);
    }
  };

  trace.step = (nowMs, frame, geometry) => {
    const elapsedMs = lastMs === 0 ? FRAME_MS : nowMs - lastMs;
    lastMs = nowMs;
    lastStepSeconds = clamp(elapsedMs / MS_PER_SECOND, 0, MAX_STEP_SECONDS);
    trace.count = pointCountFor(geometry.width);
    if (!reducedMotion) {
      for (const [index, partial] of WAVE_PARTIALS.entries()) {
        phases[index] =
          ((phases[index] ?? 0) + partial.drift * lastStepSeconds) % TWO_PI;
      }
    }
    fillTargets(nowMs, frame);
    signalPeak = peakOf(targets, trace.count);
    const active = !loading && signalPeak >= SIGNAL_THRESHOLD;
    ease(elapsedMs);
    if (!reducedMotion) {
      animate(nowMs, geometry);
    }
    previousSignalPeak = signalPeak;
    primed = true;
    return active;
  };

  return trace;
};

interface PaintInput {
  trace: ElectricTrace;
  geometry: ElectricTraceGeometry;
  colors: ElectricColors;
  intensity: number;
  nowMs: number;
}

const traceLine = (
  context: CanvasRenderingContext2D,
  { geometry, intensity, trace }: PaintInput,
  scale: number,
  crackleShare: number
) => {
  context.beginPath();
  for (let point = 0; point < trace.count; point += 1) {
    const x = xOf(geometry, trace.count, point);
    const y = yOf(trace, geometry, intensity, point, scale, crackleShare);
    if (point === 0) {
      context.moveTo(x, y);
    } else {
      context.lineTo(x, y);
    }
  }
};

/** The band between two scaled copies of the line, for the tall glow. */
const traceRibbon = (
  context: CanvasRenderingContext2D,
  { geometry, intensity, trace }: PaintInput,
  inner: number,
  outer: number
) => {
  context.beginPath();
  for (let point = 0; point < trace.count; point += 1) {
    const x = xOf(geometry, trace.count, point);
    const y = yOf(trace, geometry, intensity, point, outer, 0.35);
    if (point === 0) {
      context.moveTo(x, y);
    } else {
      context.lineTo(x, y);
    }
  }
  for (let point = trace.count - 1; point >= 0; point -= 1) {
    const x = xOf(geometry, trace.count, point);
    context.lineTo(x, yOf(trace, geometry, intensity, point, inner, 0.35));
  }
  context.closePath();
};

const traceBranch = (
  context: CanvasRenderingContext2D,
  { geometry, intensity, trace }: PaintInput,
  branch: ElectricBranch
) => {
  const point = Math.min(branch.point, trace.count - 1);
  const x = xOf(geometry, trace.count, point);
  const y = yOf(trace, geometry, intensity, point);
  const dx = branch.lean * branch.length;
  const dy = -branch.direction * branch.length;
  const length = Math.hypot(dx, dy) || 1;
  const bow = length * BRANCH_BOW;
  context.beginPath();
  for (let index = 0; index < BRANCH_POINTS; index += 1) {
    const progress = index / (BRANCH_POINTS - 1);
    const bend = (branch.jitter[index] ?? 0) * bow;
    const px = x + dx * progress - (dy / length) * bend;
    const py = y + dy * progress + (dx / length) * bend;
    if (index === 0) {
      context.moveTo(px, py);
    } else {
      context.lineTo(px, py);
    }
  }
};

const strokeBranches = (
  context: CanvasRenderingContext2D,
  input: PaintInput,
  lineWidth: number,
  alpha: number
) => {
  context.lineWidth = lineWidth;
  for (const branch of input.trace.branches) {
    if (branch.lifeMs > 0) {
      const age = (input.nowMs - branch.bornMs) / branch.lifeMs;
      context.globalAlpha = clamp(alpha * (1 - age * age), 0, 1);
      traceBranch(context, input, branch);
      context.stroke();
    }
  }
};

/** Cuts the ends away with a gradient, so the line fades in and out. */
const fadeEnds = (
  context: CanvasRenderingContext2D,
  geometry: ElectricTraceGeometry
) => {
  const edge = Math.min(FADE_PX, geometry.width / 2);
  context.globalAlpha = 1;
  context.globalCompositeOperation = "destination-out";
  const left = context.createLinearGradient(0, 0, edge, 0);
  left.addColorStop(0, "rgba(0, 0, 0, 1)");
  left.addColorStop(1, "rgba(0, 0, 0, 0)");
  context.fillStyle = left;
  context.fillRect(0, 0, edge, geometry.height);
  const right = context.createLinearGradient(
    geometry.width - edge,
    0,
    geometry.width,
    0
  );
  right.addColorStop(0, "rgba(0, 0, 0, 0)");
  right.addColorStop(1, "rgba(0, 0, 0, 1)");
  context.fillStyle = right;
  context.fillRect(geometry.width - edge, 0, edge, geometry.height);
  context.globalCompositeOperation = "source-over";
};

/**
 * The tall, soft glow: bands that echo the wave above and below it, and a
 * wide stroke. The canvas is blurred with CSS, far cheaper than `shadowBlur`.
 */
const paintGlow = (context: CanvasRenderingContext2D, input: PaintInput) => {
  const { geometry, trace } = input;
  const strength = (0.4 + 0.6 * trace.loudness) * trace.flicker;
  context.fillStyle = input.colors.glow;
  context.strokeStyle = input.colors.glow;
  context.globalAlpha = 0.22 * strength;
  traceRibbon(context, input, 0.2, 2.6);
  context.fill();
  context.globalAlpha = 0.4 * strength;
  traceRibbon(context, input, 0.6, 1.6);
  context.fill();
  context.globalAlpha = 0.6 * strength;
  context.lineWidth = geometry.lineWidth * 4;
  traceLine(context, input, 1, 0.6);
  context.stroke();
  strokeBranches(context, input, geometry.lineWidth * 1.5, 0.7);
};

const paintMain = (context: CanvasRenderingContext2D, input: PaintInput) => {
  const { geometry, trace } = input;
  context.strokeStyle = input.colors.body;
  context.globalAlpha = 0.9 * trace.flicker;
  context.lineWidth = geometry.lineWidth;
  traceLine(context, input, 1, 0.6);
  context.stroke();
  context.lineJoin = "miter";
  strokeBranches(context, input, Math.max(1.5, geometry.lineWidth * 0.5), 0.7);
  context.strokeStyle = input.colors.core;
  context.globalAlpha = trace.flicker;
  context.lineWidth = Math.max(1.5, geometry.lineWidth * 0.5);
  traceLine(context, input, 1, 1);
  context.stroke();
  strokeBranches(context, input, 1, 1);
  strokeSparks(
    context,
    trace.sparks,
    Math.max(1, geometry.lineWidth * 0.4),
    SPARK_TRAIL_SECONDS
  );
};

export const ElectricWaveform = ({
  source,
  mode = "wave",
  loading = false,
  intensity = DEFAULT_INTENSITY,
  arcs = true,
  sparks = true,
  sensitivity = 1,
  lineWidth = DEFAULT_LINE_WIDTH,
  fadeEdges = true,
  actionsRef,
  className,
  ref,
  ...props
}: ElectricWaveformProps) => {
  const reducedMotion = useReducedMotion();
  const rootRef = useRef<HTMLDivElement | null>(null);
  const glowRef = useRef<HTMLCanvasElement | null>(null);
  const mainRef = useRef<HTMLCanvasElement | null>(null);
  const frameRef = useRef<VisualFrame | null>(null);
  const visibleRef = useRef(true);

  const paint = useCallback((frame: VisualFrame) => {
    frameRef.current = frame;
  }, []);

  useFrameSource(source, paint);

  useImperativeHandle(
    actionsRef,
    () => ({
      clear: () => {
        frameRef.current = null;
      },
      paint,
    }),
    [paint]
  );

  useEffect(() => {
    const root = rootRef.current;
    if (!root || typeof IntersectionObserver === "undefined") {
      return;
    }
    const observer = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        visibleRef.current = entry.isIntersecting;
      }
    });
    observer.observe(root);
    return () => {
      observer.disconnect();
    };
  }, []);

  useEffect(() => {
    const root = rootRef.current;
    const main = mainRef.current;
    const glow = glowRef.current;
    const mainContext = main?.getContext("2d");
    const glowContext = glow?.getContext("2d");
    if (!(root && main && glow && mainContext && glowContext)) {
      return;
    }
    const strength = clamp(intensity, 0, 1);
    const trace = createElectricTrace({
      arcs,
      intensity: strength,
      loading,
      mode,
      reducedMotion,
      sensitivity,
      sparks,
    });
    let size: ElectricCanvasSize = {
      glowRatio: 1,
      height: 0,
      ratio: 1,
      width: 0,
    };
    let colors = readElectricColors(main, glow);
    let framesSinceColor = 0;
    let lastPaintMs = 0;
    let active = false;

    const resize = () => {
      size = fitElectricCanvases(main, glow);
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(main);

    const tick = (nowMs: number) => {
      if (!visibleRef.current || size.width === 0) {
        return;
      }
      framesSinceColor += 1;
      if (framesSinceColor >= COLOR_REFRESH_FRAMES) {
        framesSinceColor = 0;
        colors = readElectricColors(main, glow);
      }
      if (reducedMotion && nowMs - lastPaintMs < REDUCED_MOTION_INTERVAL_MS) {
        return;
      }
      lastPaintMs = nowMs;
      const geometry: ElectricTraceGeometry = {
        height: size.height,
        lineWidth,
        width: size.width,
      };
      const nextActive = trace.step(nowMs, frameRef.current, geometry);
      if (nextActive !== active) {
        active = nextActive;
        root.toggleAttribute("data-active", active);
      }
      const input: PaintInput = {
        colors,
        geometry,
        intensity: reducedMotion ? 0 : strength,
        nowMs,
        trace,
      };
      clearElectricCanvas(glowContext, size, size.glowRatio);
      paintGlow(glowContext, input);
      clearElectricCanvas(mainContext, size, size.ratio);
      paintMain(mainContext, input);
      if (fadeEdges) {
        fadeEnds(glowContext, geometry);
        fadeEnds(mainContext, geometry);
      }
    };

    const unsubscribe = subscribeFrame(tick);
    return () => {
      unsubscribe();
      observer.disconnect();
      delete root.dataset.active;
    };
  }, [
    arcs,
    fadeEdges,
    intensity,
    lineWidth,
    loading,
    mode,
    reducedMotion,
    sensitivity,
    sparks,
  ]);

  const setRootRef = useCallback(
    (node: HTMLDivElement | null) => {
      rootRef.current = node;
      if (typeof ref === "function") {
        ref(node);
      } else if (ref) {
        ref.current = node;
      }
    },
    [ref]
  );

  return (
    <div
      aria-label="Audio waveform"
      className={cn("relative h-24 w-full", className)}
      data-loading={loading ? "" : undefined}
      data-mode={mode}
      data-slot="electric-waveform"
      role="img"
      {...props}
      ref={setRootRef}
    >
      <canvas
        aria-hidden
        className={ELECTRIC_GLOW_CLASS}
        data-slot="electric-waveform-glow"
        ref={glowRef}
      />
      <canvas
        aria-hidden
        className={ELECTRIC_CANVAS_CLASS}
        data-slot="electric-waveform-canvas"
        ref={mainRef}
      />
    </div>
  );
};
