/** A WheelEvent fits. Chrome and Safari add the legacy deltas. */
export type WheelInput = Pick<
  WheelEvent,
  "deltaMode" | "deltaX" | "deltaY" | "timeStamp"
> & { wheelDeltaX?: number; wheelDeltaY?: number };

/** 1 for a step up (or left), -1 for a step down (or right), 0 for none. */
export type WheelStep = -1 | 0 | 1;

export type WheelStepper = (event: WheelInput) => WheelStep;

const DOM_DELTA_PIXEL = 0;
/** Chrome and Safari report a mouse-wheel notch as a multiple of 120... */
const LEGACY_NOTCH = 120;
/** ...but a macOS trackpad as -3 × the pixel delta, which can land on 120 too. */
const TRACKPAD_LEGACY_RATIO = -3;
/** Smooth-scrolling mice send a large pixel delta per notch. */
const NOTCH_MIN_PX = 50;
/** Events closer together than this belong to one stream. */
const STREAM_GAP_MS = 40;
const SMOOTH_STEP_PX = 30;
const SMOOTH_RESET_MS = 150;

/** Notches closer together than this come from a free-spinning wheel. */
const FREE_SPIN_GAP_MS = 25;
const FREE_SPIN_RUN = 3;
const UNLOCK_MS = 120;
const SHRINK_RUN = 3;
const SHRINK_UNDER_PEAK = 0.8;
const WINDOW = 4;
const WINDOW_UNDER_PREVIOUS = 0.85;
/** Momentum tails decay in integer plateaus (4, 4, 4, 3, 3, …) under half the peak. */
const TAIL_UNDER_PEAK = 0.5;
/** A rise this large while locked is a hand pushing again. */
const PUSH_MIN_PX = 4;

const isNotch = (
  deltaMode: number,
  delta: number,
  legacy: number | undefined,
  sincePrevious: number | null
) => {
  if (deltaMode !== DOM_DELTA_PIXEL) {
    return true;
  }
  if (
    legacy &&
    legacy % LEGACY_NOTCH === 0 &&
    legacy !== TRACKPAD_LEGACY_RATIO * delta
  ) {
    return true;
  }
  const paused = sincePrevious === null || sincePrevious >= STREAM_GAP_MS;
  return paused && Math.abs(delta) >= NOTCH_MIN_PX;
};

const mean = (values: number[]) =>
  values.reduce((sum, value) => sum + value, 0) / values.length;

const neverGrows = (values: number[]) =>
  values.every(
    (value, index) => index === 0 || value <= (values[index - 1] ?? 0)
  );

const strictlyShrinks = (sizes: number[]) => {
  const last = sizes.slice(-SHRINK_RUN - 1);
  return (
    last.length > SHRINK_RUN &&
    last.every((value, index) => index === 0 || value < (last[index - 1] ?? 0))
  );
};

const windowDecays = (sizes: number[]) => {
  if (sizes.length < WINDOW * 2) {
    return false;
  }
  const last = sizes.slice(-WINDOW);
  const before = sizes.slice(-WINDOW * 2, -WINDOW);
  return neverGrows(last) && mean(last) < WINDOW_UNDER_PREVIOUS * mean(before);
};

const decays = (sizes: number[], peak: number) => {
  const size = sizes.at(-1) ?? 0;
  return (
    (size < SHRINK_UNDER_PEAK * peak && strictlyShrinks(sizes)) ||
    (size <= TAIL_UNDER_PEAK * peak && windowDecays(sizes))
  );
};

const isDense = (sincePrevious: number | null, notch: boolean) =>
  sincePrevious !== null &&
  sincePrevious < (notch ? FREE_SPIN_GAP_MS : STREAM_GAP_MS);

/**
 * Momentum (trackpad inertia, a free-spinning wheel) decays after a gesture's
 * peak, in a dense stream. The peak restarts on a sharp drop, so a hand that
 * slows down is not read as momentum.
 */
const createMomentumFilter = () => {
  let direction = 0;
  let locked = false;
  let peak = 0;
  let sizes: number[] = [];
  let denseNotches = 0;
  let lowRun = 0;

  const restart = (sense: number) => {
    direction = sense;
    locked = false;
    peak = 0;
    sizes = [];
    denseNotches = 0;
    lowRun = 0;
  };

  return (
    size: number,
    sense: number,
    sincePrevious: number | null,
    notch: boolean
  ) => {
    const paused = sincePrevious === null || sincePrevious > UNLOCK_MS;
    if (paused || sense !== direction) {
      restart(sense);
    } else if (locked) {
      const previous = sizes.at(-1) ?? 0;
      if (size < PUSH_MIN_PX || size <= previous) {
        sizes = [size];
        return true;
      }
      restart(sense);
    }
    const previous = sizes.at(-1);
    if (previous !== undefined && size < TAIL_UNDER_PEAK * previous) {
      sizes = [];
      peak = 0;
      lowRun = 0;
    }
    sizes = [...sizes, size].slice(-WINDOW * 2);
    peak = Math.max(peak, size);
    const dense = isDense(sincePrevious, notch);
    denseNotches = dense && notch ? denseNotches + 1 : 0;
    lowRun = size <= TAIL_UNDER_PEAK * peak ? lowRun + 1 : 0;
    locked =
      dense &&
      (decays(sizes, peak) ||
        lowRun >= WINDOW ||
        denseNotches >= FREE_SPIN_RUN);
    return locked;
  };
};

/**
 * Turns wheel events into steps. A mouse-wheel notch is one step. Trackpad
 * deltas add up to a step every 30 px, at most one per event. Momentum after
 * the hand lets go is ignored.
 */
export const createWheelStepper = (): WheelStepper => {
  const isMomentum = createMomentumFilter();
  let lastAt: number | null = null;
  let accumulated = 0;

  return (event) => {
    // Firefox reports lines only when deltaMode is read before the deltas.
    const { deltaMode } = event;
    const vertical = event.deltaY !== 0;
    const delta = vertical ? event.deltaY : event.deltaX;
    if (delta === 0) {
      return 0;
    }
    const sincePrevious = lastAt === null ? null : event.timeStamp - lastAt;
    lastAt = event.timeStamp;
    const sense = Math.sign(delta);
    const step: WheelStep = delta < 0 ? 1 : -1;
    const legacy = vertical ? event.wheelDeltaY : event.wheelDeltaX;
    const notch = isNotch(deltaMode, delta, legacy, sincePrevious);
    if (isMomentum(Math.abs(delta), sense, sincePrevious, notch)) {
      accumulated = 0;
      return 0;
    }
    if (notch) {
      accumulated = 0;
      return step;
    }
    const stale = sincePrevious === null || sincePrevious > SMOOTH_RESET_MS;
    if (stale || Math.sign(accumulated) !== sense) {
      accumulated = 0;
    }
    accumulated += delta;
    if (Math.abs(accumulated) < SMOOTH_STEP_PX) {
      return 0;
    }
    const remainder = Math.abs(accumulated) - SMOOTH_STEP_PX;
    accumulated = sense * Math.min(remainder, SMOOTH_STEP_PX - 1);
    return step;
  };
};
