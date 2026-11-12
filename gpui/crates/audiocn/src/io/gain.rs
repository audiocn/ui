//! Port of `hooks/use-gain-node.ts`: a gain stage wired from an input to a
//! destination, its gain ramped (`setTargetAtTime`) so changes never click.
//! The first gain is set, not ramped, so a node that starts muted is never
//! heard at unity.
//!
//! ```ignore
//! // Monitor a microphone at −6 dB on the speakers.
//! let node = GainNode::new(GainNodeOptions {
//!     input: Some(AudioInput::Microphone(mic.clone())),
//!     gain: db_to_gain(-6.0),
//!     ..Default::default()
//! })?;
//! node.set_gain(0.0); // ramps to silence over ~3 × 10 ms
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;

use super::Result;
use super::context::{AtomicF32, AudioInput, AudioSource, Bus, Destination};

/// `DEFAULT_TIME_CONSTANT`: the ramp time constant, in seconds.
pub const DEFAULT_TIME_CONSTANT: f32 = 0.01;

/// `setTargetAtTime`: the value `seconds` after starting a ramp from `start`
/// towards `target` with time constant `time_constant`.
pub fn set_target_at_time_value(start: f32, target: f32, time_constant: f32, seconds: f32) -> f32 {
    if time_constant <= 0.0 {
        return target;
    }
    target + (start - target) * (-seconds / time_constant).exp()
}

/// The per-sample smoothing coefficient of a `setTargetAtTime` ramp.
pub fn ramp_coefficient(time_constant: f32, sample_rate: u32) -> f32 {
    if time_constant <= 0.0 || sample_rate == 0 {
        return 1.0;
    }
    1.0 - (-1.0 / (time_constant * sample_rate as f32)).exp()
}

/// A gain `AudioParam` shared with the audio thread.
#[derive(Debug)]
pub struct GainParam {
    target: AtomicF32,
    time_constant: AtomicF32,
    jump: AtomicBool,
}

impl GainParam {
    /// Starts at `value` (no ramp).
    pub fn new(value: f32, time_constant: f32) -> Self {
        Self {
            target: AtomicF32::new(value),
            time_constant: AtomicF32::new(time_constant),
            jump: AtomicBool::new(true),
        }
    }

    /// `setTargetAtTime(value, now, timeConstant)`.
    pub fn set_target(&self, value: f32) {
        self.target.store(value);
    }

    /// `setValueAtTime(value, now)`: jumps without a ramp.
    pub fn set_value(&self, value: f32) {
        self.target.store(value);
        self.jump.store(true, Ordering::Release);
    }

    pub fn set_time_constant(&self, time_constant: f32) {
        self.time_constant.store(time_constant);
    }

    /// The value the param is heading to.
    pub fn target(&self) -> f32 {
        self.target.load()
    }
}

/// The audio-thread side of a [`GainParam`].
pub(crate) struct Ramp {
    param: Arc<GainParam>,
    current: f32,
    coefficient: f32,
    target: f32,
}

impl Ramp {
    pub(crate) fn new(param: Arc<GainParam>) -> Self {
        let current = param.target();
        Self {
            param,
            current,
            coefficient: 1.0,
            target: current,
        }
    }

    /// Reads the param once per block.
    pub(crate) fn begin(&mut self, sample_rate: u32) {
        self.target = self.param.target();
        if self.param.jump.swap(false, Ordering::AcqRel) {
            self.current = self.target;
        }
        self.coefficient = ramp_coefficient(self.param.time_constant.load(), sample_rate);
    }

    /// The value for the next sample.
    pub(crate) fn next(&mut self) -> f32 {
        self.current += (self.target - self.current) * self.coefficient;
        self.current
    }
}

/// `UseGainNodeOptions`.
pub struct GainNodeOptions {
    /// What feeds the node. More can be connected to [`GainNode::input`].
    pub input: Option<AudioInput>,
    /// Linear gain. Default 1.
    pub gain: f32,
    /// Default the speakers; `Destination::None` to route it yourself with
    /// [`GainNode::take_output`].
    pub destination: Destination,
    /// Ramp time constant, in seconds. Default 0.01.
    pub time_constant: f32,
}

impl Default for GainNodeOptions {
    fn default() -> Self {
        Self {
            input: None,
            gain: 1.0,
            destination: Destination::Speakers,
            time_constant: DEFAULT_TIME_CONSTANT,
        }
    }
}

/// A gain node (`useGainNode`). Disconnects when dropped.
pub struct GainNode {
    param: Arc<GainParam>,
    input: Bus,
    alive: Arc<AtomicBool>,
    output: Mutex<Option<Box<dyn AudioSource>>>,
}

impl GainNode {
    pub fn new(options: GainNodeOptions) -> Result<GainNode> {
        let param = Arc::new(GainParam::new(options.gain, options.time_constant));
        let input = Bus::new();
        if let Some(source) = options.input {
            input.add_boxed(source.into_source());
        }
        let alive = Arc::new(AtomicBool::new(true));
        let source = GainSource {
            input: input.clone(),
            ramp: Ramp::new(param.clone()),
            alive: alive.clone(),
            scratch: Vec::new(),
        };
        let output = options.destination.connect(Box::new(source))?;
        Ok(GainNode {
            param,
            input,
            alive,
            output: Mutex::new(output),
        })
    }

    /// Ramps to `gain` (linear).
    pub fn set_gain(&self, gain: f32) {
        self.param.set_target(gain);
    }

    /// The gain the node is heading to.
    pub fn gain(&self) -> f32 {
        self.param.target()
    }

    pub fn set_time_constant(&self, time_constant: f32) {
        self.param.set_time_constant(time_constant);
    }

    /// The node's input: connect sources here.
    pub fn input(&self) -> Bus {
        self.input.clone()
    }

    /// Connects another input.
    pub fn connect(&self, input: AudioInput) {
        self.input.add_boxed(input.into_source());
    }

    /// The node's output, once, when it was built with `Destination::None`.
    pub fn take_output(&self) -> Option<Box<dyn AudioSource>> {
        self.output.lock().take()
    }

    /// The shared param (e.g. for automation from another thread).
    pub fn param(&self) -> Arc<GainParam> {
        self.param.clone()
    }

    /// Disconnects from the destination. The node is silent afterwards.
    pub fn disconnect(&self) {
        self.alive.store(false, Ordering::Release);
    }
}

impl Drop for GainNode {
    fn drop(&mut self) {
        self.disconnect();
    }
}

struct GainSource {
    input: Bus,
    ramp: Ramp,
    alive: Arc<AtomicBool>,
    scratch: Vec<f32>,
}

impl AudioSource for GainSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) -> bool {
        if !self.alive.load(Ordering::Acquire) {
            return false;
        }
        self.scratch.clear();
        self.scratch.resize(out.len(), 0.0);
        self.input.render(&mut self.scratch, sample_rate);
        self.ramp.begin(sample_rate);
        for (out, pair) in out
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .zip(self.scratch.as_chunks::<2>().0.iter())
        {
            let gain = self.ramp.next();
            out[0] += pair[0] * gain;
            out[1] += pair[1] * gain;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::context::FnSource;

    fn ones() -> FnSource<impl FnMut(&mut [f32], u32) -> bool + Send + 'static> {
        FnSource(|out: &mut [f32], _| {
            out.iter_mut().for_each(|s| *s += 1.0);
            true
        })
    }

    #[test]
    fn starts_at_its_gain_then_ramps_later_changes() {
        let node = GainNode::new(GainNodeOptions {
            input: Some(AudioInput::source(ones())),
            gain: 0.0,
            destination: Destination::None,
            ..Default::default()
        })
        .unwrap();
        let mut output = node.take_output().unwrap();
        let mut out = vec![0.0f32; 2 * 480];
        output.render(&mut out, 48_000);
        assert!(out.iter().all(|s| *s == 0.0), "never heard at unity");

        node.set_gain(0.5);
        let mut out = vec![0.0f32; 2 * 480];
        output.render(&mut out, 48_000);
        // 10 ms into a 10 ms time-constant ramp: 63 % of the way.
        let expected = set_target_at_time_value(0.0, 0.5, 0.01, 0.01);
        assert!((out[2 * 479] - expected).abs() < 1e-3, "{}", out[2 * 479]);
        assert!(out[0] > 0.0 && out[0] < 0.01);
    }

    #[test]
    fn dropping_the_node_disconnects_it() {
        let bus = Bus::new();
        let node = GainNode::new(GainNodeOptions {
            input: Some(AudioInput::source(ones())),
            destination: Destination::Bus(bus.clone()),
            ..Default::default()
        })
        .unwrap();
        let mut out = vec![0.0f32; 8];
        bus.render(&mut out, 48_000);
        assert_eq!(out, vec![1.0; 8]);
        drop(node);
        bus.render(&mut out, 48_000);
        assert!(bus.is_empty());
    }

    #[test]
    fn ramp_matches_set_target_at_time() {
        assert_eq!(set_target_at_time_value(1.0, 0.0, 0.0, 0.5), 0.0);
        assert!((set_target_at_time_value(1.0, 0.0, 0.1, 0.3) - (-3.0f32).exp()).abs() < 1e-6);
        assert_eq!(ramp_coefficient(0.0, 48_000), 1.0);
    }
}
