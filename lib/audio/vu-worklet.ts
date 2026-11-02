import {
  NEEDLE_REST,
  needleConstants,
  stepNeedle,
  VU_BALLISTICS,
  VU_SCALE,
} from "@/lib/audio/needle";

/**
 * Unique per module evaluation, so a reloaded module (HMR) never clashes with
 * an earlier registration. Not crypto.randomUUID: it throws outside secure
 * contexts, and this module loads on every page that renders a VuMeter.
 */
export const VU_PROCESSOR_NAME = `audiocn-vu-needle-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;

/**
 * The VU needle as an AudioWorklet module: |x| → resonant low-pass with
 * stops → scale position, every sample. Kept as source text so registry
 * users need no worklet bundler setup; the physics is the tested code in
 * `needle.ts`, copied in as text.
 */
export const VU_PROCESSOR_SOURCE = `
const needleConstants = ${needleConstants.toString()};
const stepNeedle = ${stepNeedle.toString()};
const PARAMS = ['lowPassCutoffHz', 'lowPassQ', 'rebound', 'referenceDb', 'minDb', 'maxDb'];

class VuNeedleProcessor extends AudioWorkletProcessor {
  static get parameterDescriptors() {
    const param = (name, defaultValue, minValue, maxValue) =>
      ({ name, defaultValue, minValue, maxValue, automationRate: 'k-rate' });
    return [
      param('lowPassCutoffHz', ${VU_BALLISTICS.lowPassCutoffHz}, 0.1, 100),
      param('lowPassQ', ${VU_BALLISTICS.lowPassQ}, 0.1, 10),
      param('rebound', ${VU_BALLISTICS.rebound}, 0, 1),
      param('referenceDb', ${VU_SCALE.referenceDb}, -60, 0),
      param('minDb', ${VU_SCALE.minDb}, -60, 0),
      param('maxDb', ${VU_SCALE.maxDb}, 0, 24),
    ];
  }

  constructor(options) {
    super();
    const { channel, intervalMs } = options.processorOptions;
    this.channel = channel;
    this.period = Math.max(1, Math.round((sampleRate * intervalMs) / 1000));
    this.count = 0;
    this.state = { position: ${NEEDLE_REST}, velocity: 0 };
    this.values = new Float64Array(PARAMS.length).fill(Number.NaN);
    this.constants = null;
    this.active = true;
    this.port.onmessage = (event) => {
      if (event.data === 'reset') {
        this.state.position = ${NEEDLE_REST};
        this.state.velocity = 0;
      } else if (event.data === 'stop') {
        this.active = false;
      }
    };
  }

  /** Recomputes the coefficients only when a param changes, without allocating. */
  constantsFor(parameters) {
    let changed = false;
    for (let i = 0; i < PARAMS.length; i++) {
      const value = parameters[PARAMS[i]][0];
      if (value !== this.values[i]) {
        this.values[i] = value;
        changed = true;
      }
    }
    if (changed) {
      const [lowPassCutoffHz, lowPassQ, rebound, referenceDb, minDb, maxDb] = this.values;
      this.constants = needleConstants(
        { lowPassCutoffHz, lowPassQ, rebound },
        { maxDb, minDb, referenceDb }
      );
    }
    return this.constants;
  }

  process(inputs, outputs, parameters) {
    if (!this.active) return false;
    const constants = this.constantsFor(parameters);
    const channels = inputs[0] ?? [];
    const picked = this.channel === 'max'
      ? null
      : channels[Math.min(this.channel, channels.length - 1)];
    const length = outputs[0]?.[0]?.length ?? 128;
    const dt = 1 / sampleRate;
    for (let i = 0; i < length; i++) {
      let drive = 0;
      if (picked) {
        drive = Math.abs(picked[i]);
      } else {
        for (const channel of channels) {
          const sample = Math.abs(channel[i]);
          if (sample > drive) drive = sample;
        }
      }
      stepNeedle(this.state, drive, constants, dt);
      if (++this.count === this.period) {
        this.count = 0;
        this.port.postMessage(this.state.position);
      }
    }
    // The output stays silent: this is a measurement tap.
    return true;
  }
}

registerProcessor('${VU_PROCESSOR_NAME}', VuNeedleProcessor);
`;

const modules = new WeakMap<BaseAudioContext, Promise<void>>();

/** Adds the processor to a context once. */
export const loadVuProcessor = (context: BaseAudioContext): Promise<void> => {
  const existing = modules.get(context);
  if (existing) {
    return existing;
  }
  const load = async () => {
    if (!context.audioWorklet) {
      throw new Error(
        "AudioWorklet is unavailable: the VU needle needs a secure context (HTTPS or localhost)."
      );
    }
    const url = URL.createObjectURL(
      new Blob([VU_PROCESSOR_SOURCE], { type: "text/javascript" })
    );
    try {
      await context.audioWorklet.addModule(url);
    } catch (error) {
      modules.delete(context);
      throw error;
    } finally {
      URL.revokeObjectURL(url);
    }
  };
  const pending = load();
  modules.set(context, pending);
  return pending;
};
