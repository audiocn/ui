import { clamp } from "@/lib/audio/decibels";

/** How a meter needle moves. */
export interface NeedleOptions {
  /** Time to reach 99% of a step, in milliseconds. 0 follows the target exactly. */
  riseMs: number;
  /** How far past a step the needle swings, as a fraction of the step. 0 never overshoots. */
  overshoot: number;
}

/** `vu` is the standard volume indicator: 99% in 300 ms, 1.5% overshoot. */
export const NEEDLE_BALLISTICS = {
  instant: { overshoot: 0, riseMs: 0 },
  vu: { overshoot: 0.015, riseMs: 300 },
} as const satisfies Record<string, NeedleOptions>;

export type NeedlePreset = keyof typeof NEEDLE_BALLISTICS;

export type NeedleInput = NeedlePreset | Partial<NeedleOptions>;

export const resolveNeedle = (input: NeedleInput = "vu"): NeedleOptions => {
  if (typeof input === "string") {
    return NEEDLE_BALLISTICS[input];
  }
  return { ...NEEDLE_BALLISTICS.vu, ...input };
};

/** The pins a needle can't move past, in scale positions. */
export interface NeedleStops {
  min: number;
  max: number;
}

export interface Needle {
  /** Moves toward `target`, a scale position, and returns where the needle is. */
  step: (target: number, nowMs: number) => number;
  /** Puts the needle back at rest against the low pin. */
  reset: () => void;
}

const SETTLED = 0.99;
const MAX_OVERSHOOT = 0.5;
const SEARCH_STEP = 0.01;
const SEARCH_ROUNDS = 40;
const SUBSTEP_MS = 1;
const UNBOUNDED: NeedleStops = {
  max: Number.POSITIVE_INFINITY,
  min: Number.NEGATIVE_INFINITY,
};
/** After a longer gap (a hidden tab, an off-screen meter), jump to the target. */
const MAX_GAP_MS = 500;

/** The damping ratio that gives this much overshoot on a step. */
const dampingFor = (overshoot: number) => {
  if (overshoot <= 0) {
    return 1;
  }
  const decay = Math.log(Math.min(overshoot, MAX_OVERSHOOT));
  return -decay / Math.hypot(Math.PI, decay);
};

/** A unit step response at natural frequency 1, at time `t`. */
const stepResponse = (damping: number, t: number) => {
  if (damping >= 1) {
    return 1 - Math.exp(-t) * (1 + t);
  }
  const ringing = Math.sqrt(1 - damping ** 2);
  return (
    1 -
    Math.exp(-damping * t) *
      (Math.cos(ringing * t) + (damping / ringing) * Math.sin(ringing * t))
  );
};

/** When the unit step response first reaches 99%, at natural frequency 1. */
const settleTime = (damping: number) => {
  let high = SEARCH_STEP;
  while (stepResponse(damping, high) < SETTLED) {
    high += SEARCH_STEP;
  }
  let low = high - SEARCH_STEP;
  for (let round = 0; round < SEARCH_ROUNDS; round += 1) {
    const middle = (low + high) / 2;
    if (stepResponse(damping, middle) < SETTLED) {
      low = middle;
    } else {
      high = middle;
    }
  }
  return high;
};

/**
 * Creates a meter needle: a damped spring that swings toward its target,
 * overshoots a little and settles, and stops against a pin at each end of
 * the scale. Feed it positions that are linear in amplitude, as a real
 * movement is.
 */
export const createNeedle = (
  input?: NeedleInput,
  stops: NeedleStops = UNBOUNDED
): Needle => {
  const { overshoot, riseMs } = resolveNeedle(input);
  const damping = dampingFor(overshoot);
  // Radians per millisecond, so the step response reaches 99% at riseMs.
  const frequency = riseMs > 0 ? settleTime(damping) / riseMs : 0;
  const rest = Number.isFinite(stops.min) ? stops.min : null;
  let position: number | null = rest;
  let velocity = 0;
  let lastMs: number | null = null;

  const pin = (next: number) => {
    const pinned = clamp(next, stops.min, stops.max);
    if (pinned !== next) {
      velocity = 0;
    }
    return pinned;
  };

  const reset = () => {
    position = rest;
    velocity = 0;
    lastMs = null;
  };

  const step = (target: number, nowMs: number) => {
    const elapsedMs = lastMs === null ? 0 : Math.max(0, nowMs - lastMs);
    lastMs = nowMs;

    if (position === null || frequency === 0 || elapsedMs > MAX_GAP_MS) {
      velocity = 0;
      position = pin(target);
      return position;
    }

    let current = position;
    let remainingMs = elapsedMs;
    while (remainingMs > 0) {
      const dt = Math.min(SUBSTEP_MS, remainingMs);
      const acceleration =
        frequency ** 2 * (target - current) -
        2 * damping * frequency * velocity;
      // Semi-implicit Euler: speed first, then position, so it stays stable.
      velocity += acceleration * dt;
      current = pin(current + velocity * dt);
      remainingMs -= dt;
    }
    position = current;
    return position;
  };

  return { reset, step };
};
