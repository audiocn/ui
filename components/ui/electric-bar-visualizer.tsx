"use client";

import { useCallback, useEffect, useImperativeHandle, useRef } from "react";
import type { ComponentProps, Ref } from "react";

import { useAudioConfig } from "@/hooks/use-audio-config";
import { useFrameSource } from "@/hooks/use-frame-source";
import { useReducedMotion } from "@/hooks/use-reduced-motion";
import { createBarLevels } from "@/lib/audio/bar-levels";
import type { BarIdle } from "@/lib/audio/bar-levels";
import { clamp } from "@/lib/audio/decibels";
import { subscribeFrame } from "@/lib/audio/frame-loop";
import type { FrameSource, Orientation, VisualFrame } from "@/lib/audio/types";
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

const DEFAULT_BAR_COUNT = 16;
const DEFAULT_MIN_LEVEL = 0.08;
const DEFAULT_INTENSITY = 0.6;
const DEFAULT_BAR_WIDTH = 6;
const DEFAULT_BAR_GAP = 4;
const REDUCED_MOTION_INTERVAL_MS = 250;
const COLOR_REFRESH_FRAMES = 30;
const MS_PER_SECOND = 1000;
const FRAME_SECONDS = 1 / 60;
const MAX_STEP_SECONDS = 0.05;

/** A filament has 2^4 segments, so every split halves one cleanly. */
const FILAMENT_DEPTH = 4;
const FILAMENT_POINTS = 2 ** FILAMENT_DEPTH + 1;
/** Shortest segment in pixels before a filament drops to fewer points. */
const SEGMENT_PX = 5;
/** How much each finer split bends compared with the one before. */
const ROUGHNESS = 0.72;
/** How far a free tip wanders, relative to the jitter amplitude. */
const FREE_TIP = 0.5;
/** Each jagged shape holds at least this long, so the crackle never strobes. */
const JITTER_MS = 42;
/** Largest bend, relative to the filament's length. */
const MAX_BEND = 0.35;
/** Brightness flicker, at most 12%. */
const FLICKER = 0.12;
/** Tips start to glow above this level. */
const TIP_LEVEL = 0.25;

const MAX_ARCS = 3;
/** Arcs jump to the next bar or the one after. */
const MAX_ARC_REACH = 2;
const ARC_DEPTH = 3;
const ARC_POINTS = 2 ** ARC_DEPTH + 1;
const ARC_ATTEMPTS = 2;
/** Both neighbours must be this loud for an arc to jump between them. */
const ARC_THRESHOLD = 0.45;
const ARC_CHANCE = 0.35;
const ARC_MIN_MS = 70;
const ARC_MAX_MS = 150;
/** How far an arc bends, relative to the distance it spans. */
const ARC_BOW = 0.3;

const MAX_SPARKS = 64;
/** A rise this big in one frame throws sparks. */
const SPARK_RISE = 0.12;
const SPARK_MIN_LEVEL = 0.3;
const SPARK_COOLDOWN_MS = 90;
/** Pixels per second squared. */
const SPARK_GRAVITY = 520;
/** Pixels per second. */
const SPARK_SPEED = 180;
/** Pixels per second, sideways. */
const SPARK_SPREAD = 70;
const SPARK_MIN_MS = 260;
const SPARK_MAX_MS = 560;
const SPARK_TRAIL_SECONDS = 0.03;

type Align = "center" | "start" | "end";
type Tip = "from" | "to";

export interface ElectricBarVisualizerActions {
  /** Paint levels (0..1) directly. They are resampled to the bar count. */
  paint: (levels: ArrayLike<number>) => void;
}

export interface ElectricBarVisualizerProps extends ComponentProps<"div"> {
  /** A visual source; the bars follow its frequency bands. */
  source?: FrameSource<VisualFrame> | null;
  /** Levels, 0..1, for declarative use. */
  levels?: ArrayLike<number>;
  /** Number of bars. Bands are resampled to fit. Default 16. */
  barCount?: number;
  /** Where bars grow from. Default `center`. */
  align?: Align;
  /** Symmetric around the middle bar. Default false. */
  mirrored?: boolean;
  /** Resting bar size, 0..1. Default 0.08. */
  minLevel?: number;
  /** What the bars do with no signal. Default `static`. */
  idle?: BarIdle;
  /** Runs an arc along the bars, for connecting or thinking states. Default false. */
  loading?: boolean;
  orientation?: Orientation;
  /** How jagged and restless the filaments are, 0..1. Default 0.6. */
  intensity?: number;
  /** Arcs jump between loud neighbouring bars. Default true. */
  arcs?: boolean;
  /** Sparks fly off the tips on sudden rises. Default true. */
  sparks?: boolean;
  /** Widest bar in pixels. Default 6. */
  barWidth?: number;
  /** Gap between bars in pixels. Default 4. */
  barGap?: number;
  actionsRef?: Ref<ElectricBarVisualizerActions>;
}

/**
 * Where the bars sit, in CSS pixels. `across` runs along the row of bars and
 * `along` runs the way they grow, so both orientations share one drawing.
 */
export interface ElectricLayout {
  horizontal: boolean;
  align: Align;
  barWidth: number;
  barGap: number;
  /** Centre of the first bar, across. */
  first: number;
  /** Distance between bar centres. */
  pitch: number;
  /** Where bars grow from, along: the base, or the middle when centred. */
  base: number;
  /** Length of a bar at level 1. */
  span: number;
}

export interface ElectricLayoutOptions {
  align: Align;
  barCount: number;
  barGap: number;
  barWidth: number;
  orientation: Orientation;
}

/** Fits the bars into a canvas, narrowing bars and gaps when space is short. */
export const layoutElectricBars = (
  width: number,
  height: number,
  { align, barCount, barGap, barWidth, orientation }: ElectricLayoutOptions
): ElectricLayout => {
  const horizontal = orientation === "horizontal";
  const count = Math.max(1, barCount);
  const acrossLength = horizontal ? width : height;
  const alongLength = horizontal ? height : width;
  const pitch = Math.min(
    barWidth + barGap,
    Math.max(0, acrossLength - barWidth * 2) / count
  );
  const scale = pitch / (barWidth + barGap || 1);
  const resolvedWidth = barWidth * scale;
  const resolvedGap = barGap * scale;
  const inset = Math.min(resolvedWidth, alongLength / 4);
  const baseFor = {
    center: alongLength / 2,
    end: alongLength - inset,
    start: inset,
  };

  return {
    align,
    barGap: resolvedGap,
    barWidth: resolvedWidth,
    base: baseFor[align],
    first: (acrossLength - count * pitch + resolvedGap + resolvedWidth) / 2,
    horizontal,
    pitch,
    span: Math.max(0, alongLength - inset * 2),
  };
};

const barFrom = (layout: ElectricLayout, level: number) =>
  layout.align === "center"
    ? layout.base - (level * layout.span) / 2
    : layout.base;

const barTo = (layout: ElectricLayout, level: number) => {
  if (layout.align === "center") {
    return layout.base + (level * layout.span) / 2;
  }
  const direction = layout.align === "end" ? -1 : 1;
  return layout.base + direction * level * layout.span;
};

/** Which way a tip points, along: sparks fly that way. */
const tipDirection = (layout: ElectricLayout, tip: Tip) => {
  const growth = layout.align === "end" ? -1 : 1;
  return tip === "to" ? growth : -growth;
};

/**
 * How far a filament wanders sideways, in pixels. Louder bars crackle more,
 * and short ones never bend more than a third of their length.
 */
const jitterAmplitude = (
  layout: ElectricLayout,
  intensity: number,
  level: number
) =>
  Math.min(
    intensity * (layout.barWidth + layout.barGap) * 0.6 * (0.3 + 0.7 * level),
    level * layout.span * MAX_BEND
  );

export interface ElectricArc {
  /** The arc joins this bar and the one `reach` bars on. */
  index: number;
  reach: number;
  /** Which end of the bars it joins: `to` is the tip, `from` the base (centred bars only). */
  tip: Tip;
  bornMs: number;
  /** 0 when the arc is gone. */
  lifeMs: number;
  /** Bend at each point, in units of the arc's bow. */
  jitter: Float32Array;
}

export interface ElectricSceneOptions {
  barCount: number;
  intensity: number;
  arcs: boolean;
  sparks: boolean;
  loading: boolean;
  reducedMotion: boolean;
  /** Makes the crackle repeatable. Each scene gets its own by default. */
  seed?: number;
}

export interface ElectricScene {
  /** Sideways offset of each filament point, per bar, in units of its jitter amplitude. */
  jitter: Float32Array;
  /** Brightness of each bar, 0.88..1. */
  flicker: Float32Array;
  arcs: ElectricArc[];
  sparks: ElectricSparks;
  /** Advances the crackle, arcs and sparks to `nowMs`. */
  step: (nowMs: number, levels: Float32Array, layout: ElectricLayout) => void;
}

const loudestIndex = (levels: Float32Array) => {
  let loudest = 0;
  for (const [index, level] of levels.entries()) {
    if (level > (levels[loudest] ?? 0)) {
      loudest = index;
    }
  }
  return loudest;
};

let sceneCount = 0;

/**
 * The moving parts of the electric look, apart from any canvas: jagged
 * filaments, arcs between loud neighbours and sparks thrown off sudden rises.
 * With reduced motion it stays straight and still.
 */
export const createElectricScene = ({
  arcs,
  barCount,
  intensity,
  loading,
  reducedMotion,
  seed,
  sparks,
}: ElectricSceneOptions): ElectricScene => {
  sceneCount += 1;
  const random = createRandom(seed ?? sceneCount);
  const signed = () => random() * 2 - 1;
  const jitter = new Float32Array(barCount * FILAMENT_POINTS);
  const flicker = new Float32Array(barCount).fill(1);
  const arcList: ElectricArc[] = Array.from({ length: MAX_ARCS }, () => ({
    bornMs: 0,
    index: 0,
    jitter: new Float32Array(ARC_POINTS),
    lifeMs: 0,
    reach: 1,
    tip: "to" as Tip,
  }));
  const pool = createElectricSparks(MAX_SPARKS);
  const previous = new Float32Array(barCount);
  const lastEmitMs = new Float64Array(barCount).fill(-Infinity);
  let lastMs = 0;
  let lastJitterMs = -Infinity;
  let primed = false;

  const rerollFilaments = (centered: boolean) => {
    for (let index = 0; index < barCount; index += 1) {
      const from = centered ? signed() * FREE_TIP : 0;
      displace(
        jitter,
        index * FILAMENT_POINTS,
        FILAMENT_POINTS,
        [from, signed() * FREE_TIP],
        random,
        ROUGHNESS
      );
      flicker[index] = 1 - FLICKER * random();
    }
  };

  const startArc = (
    arc: ElectricArc,
    [index, reach]: [number, number],
    tip: Tip,
    nowMs: number
  ) => {
    arc.index = index;
    arc.reach = reach;
    arc.tip = tip;
    arc.bornMs = nowMs;
    arc.lifeMs = ARC_MIN_MS + random() * (ARC_MAX_MS - ARC_MIN_MS);
  };

  /** While loading, one arc rides the sweep across the bars. */
  const followSweep = (
    nowMs: number,
    levels: Float32Array,
    centered: boolean
  ) => {
    const peak = loudestIndex(levels);
    const leansLeft = (levels[peak - 1] ?? 0) > (levels[peak + 1] ?? 0);
    const index = clamp(leansLeft ? peak - 1 : peak, 0, barCount - 2);
    const [arc] = arcList;
    if (arc) {
      startArc(arc, [index, 1], centered ? "from" : "to", nowMs);
      arc.lifeMs = JITTER_MS * 2;
    }
  };

  const spawnArcs = (
    nowMs: number,
    levels: Float32Array,
    centered: boolean
  ) => {
    for (let attempt = 0; attempt < ARC_ATTEMPTS; attempt += 1) {
      const reach = 1 + Math.floor(random() * MAX_ARC_REACH);
      const index = Math.floor(random() * Math.max(0, barCount - reach));
      const strength = Math.min(levels[index] ?? 0, levels[index + reach] ?? 0);
      const charged =
        strength >= ARC_THRESHOLD &&
        random() < ARC_CHANCE * intensity * strength;
      const free = arcList.find((arc) => arc.lifeMs === 0);
      const taken = arcList.some(
        (arc) => arc.lifeMs > 0 && arc.index === index
      );
      if (charged && free && !taken) {
        startArc(
          free,
          [index, reach],
          centered && random() < 0.5 ? "from" : "to",
          nowMs
        );
      }
    }
  };

  const updateArcs = (
    nowMs: number,
    levels: Float32Array,
    centered: boolean
  ) => {
    if (barCount < 2) {
      return;
    }
    if (loading) {
      followSweep(nowMs, levels, centered);
    } else {
      spawnArcs(nowMs, levels, centered);
    }
    for (const arc of arcList) {
      if (arc.lifeMs > 0) {
        displace(arc.jitter, 0, ARC_POINTS, [0, 0], random, ROUGHNESS);
      }
    }
  };

  const expireArcs = (nowMs: number) => {
    for (const arc of arcList) {
      if (arc.lifeMs > 0 && nowMs - arc.bornMs >= arc.lifeMs) {
        arc.lifeMs = 0;
      }
    }
  };

  const launchSpark = (
    index: number,
    level: number,
    layout: ElectricLayout
  ) => {
    const tip: Tip =
      layout.align === "center" && random() < 0.5 ? "from" : "to";
    const along = tip === "to" ? barTo(layout, level) : barFrom(layout, level);
    const across = layout.first + index * layout.pitch;
    const speed = SPARK_SPEED * (0.5 + random()) * (0.5 + intensity);
    const alongVelocity = tipDirection(layout, tip) * speed;
    const acrossVelocity = signed() * SPARK_SPREAD;
    emitSpark(pool, {
      lifeMs: SPARK_MIN_MS + random() * (SPARK_MAX_MS - SPARK_MIN_MS),
      vx: layout.horizontal ? acrossVelocity : alongVelocity,
      vy: layout.horizontal ? alongVelocity : acrossVelocity,
      x: layout.horizontal ? across : along,
      y: layout.horizontal ? along : across,
    });
  };

  const emitSparks = (
    nowMs: number,
    levels: Float32Array,
    layout: ElectricLayout
  ) => {
    for (const [index, level] of levels.entries()) {
      const rise = level - (previous[index] ?? 0);
      const rested = nowMs - (lastEmitMs[index] ?? 0) >= SPARK_COOLDOWN_MS;
      if (rise >= SPARK_RISE && level >= SPARK_MIN_LEVEL && rested) {
        lastEmitMs[index] = nowMs;
        const count = 2 + Math.floor(random() * 3);
        for (let spark = 0; spark < count; spark += 1) {
          launchSpark(index, level, layout);
        }
      }
    }
  };

  const step = (
    nowMs: number,
    levels: Float32Array,
    layout: ElectricLayout
  ) => {
    const seconds =
      lastMs === 0
        ? FRAME_SECONDS
        : clamp((nowMs - lastMs) / MS_PER_SECOND, 0, MAX_STEP_SECONDS);
    lastMs = nowMs;
    if (reducedMotion) {
      return;
    }
    const centered = layout.align === "center";
    if (nowMs - lastJitterMs >= JITTER_MS) {
      lastJitterMs = nowMs;
      rerollFilaments(centered);
      if (arcs) {
        updateArcs(nowMs, levels, centered);
      }
    }
    expireArcs(nowMs);
    if (sparks) {
      if (primed) {
        emitSparks(nowMs, levels, layout);
      }
      moveSparks(pool, seconds, SPARK_GRAVITY);
    }
    previous.set(levels);
    primed = true;
  };

  return { arcs: arcList, flicker, jitter, sparks: pool, step };
};

interface PaintInput {
  scene: ElectricScene;
  layout: ElectricLayout;
  levels: Float32Array;
  colors: ElectricColors;
  intensity: number;
  nowMs: number;
}

const plot = (
  context: CanvasRenderingContext2D,
  layout: ElectricLayout,
  across: number,
  along: number,
  move: boolean
) => {
  const x = layout.horizontal ? across : along;
  const y = layout.horizontal ? along : across;
  if (move) {
    context.moveTo(x, y);
  } else {
    context.lineTo(x, y);
  }
};

/** Short filaments use fewer points, so they stay a line and not a scribble. */
const strideFor = (length: number) => {
  const depth = clamp(
    Math.ceil(Math.log2(Math.max(1, length) / SEGMENT_PX)),
    1,
    FILAMENT_DEPTH
  );
  return 2 ** (FILAMENT_DEPTH - depth);
};

/** How one pass draws the filaments. */
interface FilamentPass {
  /** Line width, relative to the bar width. */
  width: number;
  /** Least line width in pixels. */
  minWidth: number;
  /** Share of the jitter the pass follows: the sheath stays close to a bar while the core crackles. */
  bend: number;
  join: CanvasLineJoin;
  alpha: (level: number) => number;
}

const GLOW_PASS: FilamentPass = {
  alpha: (level) => 0.1 + 0.35 * level,
  bend: 0.35,
  join: "round",
  minWidth: 2,
  width: 1.6,
};

const SHEATH_PASS: FilamentPass = {
  alpha: (level) => 0.35 + 0.35 * level,
  bend: 0.35,
  join: "round",
  minWidth: 1,
  width: 1,
};

const CORE_PASS: FilamentPass = {
  alpha: (level) => 0.8 + 0.2 * level,
  bend: 1,
  join: "miter",
  minWidth: 1.25,
  width: 0.3,
};

const traceFilament = (
  context: CanvasRenderingContext2D,
  { intensity, layout, scene }: PaintInput,
  index: number,
  level: number,
  bendShare: number
) => {
  const across = layout.first + index * layout.pitch;
  const amplitude = jitterAmplitude(layout, intensity, level) * bendShare;
  const from = barFrom(layout, level);
  const to = barTo(layout, level);
  const stride = strideFor(Math.abs(to - from));
  const offset = index * FILAMENT_POINTS;
  context.beginPath();
  for (let point = 0; point < FILAMENT_POINTS; point += stride) {
    const along = from + ((to - from) * point) / (FILAMENT_POINTS - 1);
    const bend = (scene.jitter[offset + point] ?? 0) * amplitude;
    plot(context, layout, across + bend, along, point === 0);
  }
};

const strokeFilaments = (
  context: CanvasRenderingContext2D,
  input: PaintInput,
  pass: FilamentPass
) => {
  context.lineWidth = Math.max(
    pass.minWidth,
    input.layout.barWidth * pass.width
  );
  context.lineJoin = pass.join;
  for (const [index, level] of input.levels.entries()) {
    const flicker = input.scene.flicker[index] ?? 1;
    context.globalAlpha = clamp(pass.alpha(level) * flicker, 0, 1);
    traceFilament(context, input, index, level, pass.bend);
    context.stroke();
  }
};

/** Where a bar's tip is, jitter included, as `[across, along]`. */
const tipOf = (
  { intensity, layout, scene }: PaintInput,
  index: number,
  level: number,
  tip: Tip
): [number, number] => {
  const point = tip === "to" ? FILAMENT_POINTS - 1 : 0;
  const bend =
    (scene.jitter[index * FILAMENT_POINTS + point] ?? 0) *
    jitterAmplitude(layout, intensity, level);
  const along = tip === "to" ? barTo(layout, level) : barFrom(layout, level);
  return [layout.first + index * layout.pitch + bend, along];
};

const fillTips = (
  context: CanvasRenderingContext2D,
  input: PaintInput,
  radius: number
) => {
  const tips: Tip[] = input.layout.align === "center" ? ["from", "to"] : ["to"];
  for (const [index, level] of input.levels.entries()) {
    const heat = (level - TIP_LEVEL) / (1 - TIP_LEVEL);
    if (heat > 0) {
      context.globalAlpha =
        clamp(heat, 0, 1) * (input.scene.flicker[index] ?? 1);
      context.beginPath();
      for (const tip of tips) {
        const [across, along] = tipOf(input, index, level, tip);
        const x = input.layout.horizontal ? across : along;
        const y = input.layout.horizontal ? along : across;
        context.moveTo(x + radius, y);
        context.arc(x, y, radius, 0, Math.PI * 2);
      }
      context.fill();
    }
  }
};

const traceArc = (
  context: CanvasRenderingContext2D,
  input: PaintInput,
  arc: ElectricArc
) => {
  const { layout, levels } = input;
  const [fromAcross, fromAlong] = tipOf(
    input,
    arc.index,
    levels[arc.index] ?? 0,
    arc.tip
  );
  const [toAcross, toAlong] = tipOf(
    input,
    arc.index + arc.reach,
    levels[arc.index + arc.reach] ?? 0,
    arc.tip
  );
  const acrossSpan = toAcross - fromAcross;
  const alongSpan = toAlong - fromAlong;
  const length = Math.hypot(acrossSpan, alongSpan) || 1;
  const bow = length * ARC_BOW;
  context.beginPath();
  for (let point = 0; point < ARC_POINTS; point += 1) {
    const progress = point / (ARC_POINTS - 1);
    const bend = (arc.jitter[point] ?? 0) * bow;
    plot(
      context,
      layout,
      fromAcross + acrossSpan * progress - (alongSpan / length) * bend,
      fromAlong + alongSpan * progress + (acrossSpan / length) * bend,
      point === 0
    );
  }
};

const strokeArcs = (
  context: CanvasRenderingContext2D,
  input: PaintInput,
  lineWidth: number,
  alpha: number
) => {
  context.lineWidth = lineWidth;
  for (const arc of input.scene.arcs) {
    if (arc.lifeMs > 0) {
      const age = (input.nowMs - arc.bornMs) / arc.lifeMs;
      context.globalAlpha = clamp(alpha * (1 - age * age), 0, 1);
      traceArc(context, input, arc);
      context.stroke();
    }
  }
};

/** Wide, soft strokes. The canvas is blurred with CSS, which is far cheaper than `shadowBlur`. */
const paintGlow = (context: CanvasRenderingContext2D, input: PaintInput) => {
  const { barWidth } = input.layout;
  context.strokeStyle = input.colors.glow;
  context.fillStyle = input.colors.glow;
  strokeFilaments(context, input, GLOW_PASS);
  fillTips(context, input, barWidth * 1.2);
  context.lineJoin = "round";
  strokeArcs(context, input, barWidth, 0.7);
};

const paintMain = (context: CanvasRenderingContext2D, input: PaintInput) => {
  const { barWidth } = input.layout;
  context.strokeStyle = input.colors.body;
  strokeFilaments(context, input, SHEATH_PASS);
  context.lineJoin = "miter";
  strokeArcs(context, input, Math.max(1.5, barWidth * 0.45), 0.6);
  context.strokeStyle = input.colors.core;
  context.fillStyle = input.colors.core;
  strokeFilaments(context, input, CORE_PASS);
  strokeArcs(context, input, 1, 1);
  fillTips(context, input, Math.max(1, barWidth * 0.35));
  strokeSparks(
    context,
    input.scene.sparks,
    Math.max(1, barWidth * 0.25),
    SPARK_TRAIL_SECONDS
  );
};

export const ElectricBarVisualizer = ({
  source,
  levels,
  barCount = DEFAULT_BAR_COUNT,
  align = "center",
  mirrored = false,
  minLevel = DEFAULT_MIN_LEVEL,
  idle = "static",
  loading = false,
  orientation: orientationProp,
  intensity = DEFAULT_INTENSITY,
  arcs = true,
  sparks = true,
  barWidth = DEFAULT_BAR_WIDTH,
  barGap = DEFAULT_BAR_GAP,
  actionsRef,
  className,
  ref,
  ...props
}: ElectricBarVisualizerProps) => {
  const config = useAudioConfig();
  const orientation = orientationProp ?? config.orientation ?? "horizontal";
  const reducedMotion = useReducedMotion();
  const rootRef = useRef<HTMLDivElement | null>(null);
  const glowRef = useRef<HTMLCanvasElement | null>(null);
  const mainRef = useRef<HTMLCanvasElement | null>(null);
  const inputRef = useRef<ArrayLike<number> | null>(null);
  const visibleRef = useRef(true);

  const paint = useCallback((next: ArrayLike<number>) => {
    inputRef.current = next;
  }, []);

  useFrameSource(source, (frame) => {
    inputRef.current = frame.bands;
  });

  useEffect(() => {
    if (levels) {
      inputRef.current = levels;
    }
  }, [levels]);

  useImperativeHandle(actionsRef, () => ({ paint }), [paint]);

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
    const geometry = { align, barCount, barGap, barWidth, orientation };
    const barLevels = createBarLevels({
      barCount,
      idle,
      loading,
      minLevel,
      mirrored,
      reducedMotion,
    });
    const scene = createElectricScene({
      arcs,
      barCount,
      intensity: strength,
      loading,
      reducedMotion,
      sparks,
    });
    let size: ElectricCanvasSize = {
      glowRatio: 1,
      height: 0,
      ratio: 1,
      width: 0,
    };
    let layout = layoutElectricBars(0, 0, geometry);
    let colors = readElectricColors(main, glow);
    let framesSinceColor = 0;
    let lastPaintMs = 0;
    let active = false;

    const resize = () => {
      size = fitElectricCanvases(main, glow);
      layout = layoutElectricBars(size.width, size.height, geometry);
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
      const nextActive = barLevels.step(nowMs, inputRef.current);
      if (nextActive !== active) {
        active = nextActive;
        root.toggleAttribute("data-active", active);
      }
      scene.step(nowMs, barLevels.levels, layout);
      const input: PaintInput = {
        colors,
        intensity: reducedMotion ? 0 : strength,
        layout,
        levels: barLevels.levels,
        nowMs,
        scene,
      };
      clearElectricCanvas(glowContext, size, size.glowRatio);
      paintGlow(glowContext, input);
      clearElectricCanvas(mainContext, size, size.ratio);
      paintMain(mainContext, input);
    };

    const unsubscribe = subscribeFrame(tick);
    return () => {
      unsubscribe();
      observer.disconnect();
      delete root.dataset.active;
    };
  }, [
    align,
    arcs,
    barCount,
    barGap,
    barWidth,
    idle,
    intensity,
    loading,
    minLevel,
    mirrored,
    orientation,
    reducedMotion,
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
      aria-label="Audio visualizer"
      className={cn(
        "relative",
        orientation === "horizontal" ? "h-16 w-full" : "h-full w-16",
        className
      )}
      data-align={align}
      data-loading={loading ? "" : undefined}
      data-orientation={orientation}
      data-slot="electric-bar-visualizer"
      role="img"
      {...props}
      ref={setRootRef}
    >
      <canvas
        aria-hidden
        className={ELECTRIC_GLOW_CLASS}
        data-slot="electric-bar-visualizer-glow"
        ref={glowRef}
      />
      <canvas
        aria-hidden
        className={ELECTRIC_CANVAS_CLASS}
        data-slot="electric-bar-visualizer-canvas"
        ref={mainRef}
      />
    </div>
  );
};
