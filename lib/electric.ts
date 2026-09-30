import { clamp } from "@/lib/audio/decibels";

/**
 * Shared drawing for the electric visualizers: repeatable crackle, a spark
 * pool, canvas sizing and theme colours read from CSS.
 */

const RANDOM_PERIOD = 1_000_003;

/**
 * The blurred glow canvas under the drawing. Its colour is `--electric-glow`,
 * then `--electric`, then the text colour.
 */
export const ELECTRIC_GLOW_CLASS =
  "pointer-events-none absolute inset-0 size-full opacity-70 dark:opacity-100 [color:var(--electric-glow,var(--electric,currentColor))] blur-[var(--electric-glow-size,0.5rem)]";

/**
 * The sharp canvas on top. Its colour is `--electric`; the border colour
 * carries the core, `--electric-core` or the colour mixed toward white by
 * `--electric-heat`, so the browser resolves it. There is no border.
 */
export const ELECTRIC_CANVAS_CLASS =
  "absolute inset-0 size-full [border-color:var(--electric-core,color-mix(in_oklch,currentColor,white_var(--electric-heat,0%)))] [color:var(--electric,currentColor)] dark:[border-color:var(--electric-core,color-mix(in_oklch,currentColor,white_var(--electric-heat,70%)))]";

const fract = (value: number) => value - Math.floor(value);

/** A repeatable pseudo-random sequence, 0..1. */
export const createRandom = (seed: number): (() => number) => {
  let counter = 0;
  return () => {
    counter = (counter + 1) % RANDOM_PERIOD;
    return fract(Math.sin(seed * 12.9898 + counter * 78.233) * 43_758.5453);
  };
};

/**
 * Midpoint displacement: fills `count` offsets from `start` (`count - 1` a
 * power of two) between the two end values, big bends first and finer ones
 * on top. Each finer split bends `roughness` times the one before.
 */
export const displace = (
  out: Float32Array,
  start: number,
  count: number,
  ends: [number, number],
  random: () => number,
  roughness: number
): void => {
  const last = count - 1;
  [out[start], out[start + last]] = ends;
  let amplitude = 1;
  for (let step = last; step > 1; step /= 2) {
    const half = step / 2;
    for (let index = half; index < last; index += step) {
      const before = out[start + index - half] ?? 0;
      const after = out[start + index + half] ?? 0;
      out[start + index] =
        (before + after) / 2 + (random() * 2 - 1) * amplitude;
    }
    amplitude *= roughness;
  }
};

export interface ElectricColors {
  body: string;
  core: string;
  glow: string;
}

/** Reads the drawing colours from the two canvases, resolved by the browser. */
export const readElectricColors = (
  main: HTMLCanvasElement,
  glow: HTMLCanvasElement
): ElectricColors => {
  const style = getComputedStyle(main);
  return {
    body: style.color,
    core: style.borderTopColor || style.color,
    glow: getComputedStyle(glow).color,
  };
};

export interface ElectricCanvasSize {
  /** CSS pixels. */
  width: number;
  height: number;
  /** Device pixels per CSS pixel on the sharp canvas. */
  ratio: number;
  /** Device pixels per CSS pixel on the glow canvas. It is blurred, so it never needs more than one. */
  glowRatio: number;
}

/** Sizes both canvases to the sharp canvas's box. */
export const fitElectricCanvases = (
  main: HTMLCanvasElement,
  glow: HTMLCanvasElement
): ElectricCanvasSize => {
  const rect = main.getBoundingClientRect();
  const ratio = window.devicePixelRatio || 1;
  const glowRatio = Math.min(1, ratio);
  main.width = Math.max(1, Math.round(rect.width * ratio));
  main.height = Math.max(1, Math.round(rect.height * ratio));
  glow.width = Math.max(1, Math.round(rect.width * glowRatio));
  glow.height = Math.max(1, Math.round(rect.height * glowRatio));
  return { glowRatio, height: rect.height, ratio, width: rect.width };
};

/** Clears a canvas and sets it up for round, CSS-pixel strokes. */
export const clearElectricCanvas = (
  context: CanvasRenderingContext2D,
  size: ElectricCanvasSize,
  ratio: number
): void => {
  context.setTransform(ratio, 0, 0, ratio, 0, 0);
  context.clearRect(0, 0, size.width, size.height);
  context.globalAlpha = 1;
  context.globalCompositeOperation = "source-over";
  context.lineCap = "round";
  context.lineJoin = "round";
};

/** A fixed pool of sparks, in canvas pixels. A spark with `life` 0 is gone. */
export interface ElectricSparks {
  x: Float32Array;
  y: Float32Array;
  vx: Float32Array;
  vy: Float32Array;
  /** Milliseconds. */
  age: Float32Array;
  /** Milliseconds; 0 when the spark is gone. */
  life: Float32Array;
  /** The slot the next spark takes, replacing the oldest when full. */
  next: number;
}

export const createElectricSparks = (size: number): ElectricSparks => ({
  age: new Float32Array(size),
  life: new Float32Array(size),
  next: 0,
  vx: new Float32Array(size),
  vy: new Float32Array(size),
  x: new Float32Array(size),
  y: new Float32Array(size),
});

export interface SparkLaunch {
  x: number;
  y: number;
  /** Pixels per second. */
  vx: number;
  vy: number;
  lifeMs: number;
}

export const emitSpark = (
  sparks: ElectricSparks,
  { lifeMs, vx, vy, x, y }: SparkLaunch
): void => {
  const slot = sparks.next;
  sparks.next = (slot + 1) % sparks.life.length;
  sparks.x[slot] = x;
  sparks.y[slot] = y;
  sparks.vx[slot] = vx;
  sparks.vy[slot] = vy;
  sparks.age[slot] = 0;
  sparks.life[slot] = lifeMs;
};

/** Moves every live spark, pulled down by `gravity` in pixels per second squared. */
export const moveSparks = (
  sparks: ElectricSparks,
  seconds: number,
  gravity: number
): void => {
  for (const [slot, life] of sparks.life.entries()) {
    if (life > 0) {
      const age = (sparks.age[slot] ?? 0) + seconds * 1000;
      sparks.age[slot] = age;
      if (age >= life) {
        sparks.life[slot] = 0;
      } else {
        const vy = (sparks.vy[slot] ?? 0) + gravity * seconds;
        sparks.vy[slot] = vy;
        sparks.x[slot] =
          (sparks.x[slot] ?? 0) + (sparks.vx[slot] ?? 0) * seconds;
        sparks.y[slot] = (sparks.y[slot] ?? 0) + vy * seconds;
      }
    }
  }
};

/** Draws each live spark as a short streak that fades with age. */
export const strokeSparks = (
  context: CanvasRenderingContext2D,
  sparks: ElectricSparks,
  lineWidth: number,
  trailSeconds: number
): void => {
  context.lineWidth = lineWidth;
  for (const [slot, life] of sparks.life.entries()) {
    if (life > 0) {
      const x = sparks.x[slot] ?? 0;
      const y = sparks.y[slot] ?? 0;
      context.globalAlpha = clamp(1 - (sparks.age[slot] ?? 0) / life, 0, 1);
      context.beginPath();
      context.moveTo(
        x - (sparks.vx[slot] ?? 0) * trailSeconds,
        y - (sparks.vy[slot] ?? 0) * trailSeconds
      );
      context.lineTo(x, y);
      context.stroke();
    }
  }
};
