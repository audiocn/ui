/** How a VU needle moves: one resonant low-pass, and a bounce off the upper stop. */
export interface NeedleOptions {
  /** Cutoff of the 2nd-order low-pass, in Hz. Lower is slower. */
  lowPassCutoffHz: number;
  /** Resonance of the low-pass. 0.5 never overshoots; higher overshoots more. */
  lowPassQ: number;
  /** Share of its speed the needle keeps when it hits the upper stop, 0..1. */
  rebound: number;
}

/** 99% of a step in about 340 ms, about 1% overshoot. */
export const VU_BALLISTICS: NeedleOptions = {
  lowPassCutoffHz: 2,
  lowPassQ: 0.6,
  rebound: 0.5,
};

/** The printed scale and its calibration, as on `VuMeter`. */
export interface NeedleScale {
  /** The RMS level in dBFS that reads 0 VU. */
  referenceDb: number;
  /** Left end of the scale, in VU. */
  minDb: number;
  /** Right end of the scale, in VU. */
  maxDb: number;
}

/** 0 VU at −18 dBFS (EBU R68), printed from −10 to +3 VU. */
export const VU_SCALE: NeedleScale = {
  maxDb: 3,
  minDb: -10,
  referenceDb: -18,
};

/** Where the needle rests: just under the left end of the scale. */
export const NEEDLE_REST = -0.04;
/** The upper stop: just past the right end of the scale. */
export const NEEDLE_PIN = 1.04;

/** Coefficients for `stepNeedle`. Positions are scale positions: 0 and 1 are the ends of the arc. */
export interface NeedleConstants {
  /** Natural frequency, rad/s. */
  omega: number;
  /** Damping ratio. */
  damping: number;
  restitution: number;
  /** Scale position per unit of rectified signal. */
  gain: number;
  /** Scale position of silence. */
  offset: number;
  lower: number;
  upper: number;
}

export interface NeedleState {
  position: number;
  velocity: number;
}

export const restingNeedle = (): NeedleState => ({
  position: NEEDLE_REST,
  velocity: 0,
});

/*
 * needleConstants and stepNeedle run in the AudioWorklet too, which gets
 * their source text. Keep them self-contained: no imports, no outer names.
 */

/** Turns needle options and a calibrated scale into `stepNeedle` coefficients. */
export const needleConstants = (
  options: NeedleOptions,
  scale: NeedleScale
): NeedleConstants => {
  const low = 10 ** (scale.minDb / 20);
  const span = 10 ** (scale.maxDb / 20) - low;
  // The mean of a full-wave rectified sine is 2√2/π times its RMS.
  const referenceMean =
    ((2 * Math.SQRT2) / Math.PI) * 10 ** (scale.referenceDb / 20);
  return {
    damping: 1 / (2 * options.lowPassQ),
    gain: 1 / (referenceMean * span),
    lower: -0.04,
    offset: -low / span,
    omega: 2 * Math.PI * options.lowPassCutoffHz,
    restitution: Math.min(1, Math.max(0, options.rebound)),
    upper: 1.04,
  };
};

/**
 * Moves the needle by one sample, `dt` seconds, driven by the rectified
 * signal. A 2nd-order low-pass written as a mass on a spring, so the stops
 * can act on its position and velocity. Mutates `state`.
 */
export const stepNeedle = (
  state: NeedleState,
  drive: number,
  constants: NeedleConstants,
  dt: number
): void => {
  const { damping, gain, lower, offset, omega, restitution, upper } = constants;
  const target = drive * gain + offset;
  const acceleration =
    omega * omega * (target - state.position) -
    2 * damping * omega * state.velocity;
  // Semi-implicit Euler: speed first, then position, so it stays stable.
  const velocity = state.velocity + acceleration * dt;
  const position = state.position + velocity * dt;
  if (position > upper) {
    const reboundSpeed = Math.max(0, velocity) * restitution;
    // Rest on the stop once a bounce would be shorter than one step;
    // otherwise the integration keeps adding tiny bounces.
    if (reboundSpeed <= Math.max(0, acceleration) * dt) {
      state.position = upper;
      state.velocity = 0;
    } else {
      state.position = upper - (position - upper) * restitution;
      state.velocity = -reboundSpeed;
    }
    return;
  }
  if (position < lower) {
    state.position = lower;
    state.velocity = 0;
    return;
  }
  state.position = position;
  state.velocity = velocity;
};
