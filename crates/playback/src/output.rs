//! The audio device. It plays the mixed sound, and the number of sample frames it has played is
//! the playback clock: pictures follow the sound, never the other way round.
//!
//! Ported from FilmCraft's `apps/filmcraft/src/audio.rs` (MIT OR Apache-2.0) to cpal 0.18, for the
//! system's default output device only: Resolution has no Audio Hardware settings yet.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// The device callback: fills an interleaved buffer of `channels` channels.
pub type Fill = Box<dyn FnMut(&mut [f32], usize) + Send>;

pub struct AudioOut {
    stream: Option<cpal::Stream>,
    played: Arc<AtomicU64>,
    failed: Arc<AtomicBool>,
    rate: u32,
    channels: u16,
    device: Option<String>,
}

impl Default for AudioOut {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioOut {
    /// The default output device, not yet playing. Without a device, [`AudioOut::start`] fails
    /// and playback runs silent on the wall clock.
    pub fn new() -> Self {
        let device = cpal::default_host().default_output_device();
        let cfg = device.as_ref().and_then(|d| d.default_output_config().ok());
        Self {
            stream: None,
            played: Arc::new(AtomicU64::new(0)),
            failed: Arc::new(AtomicBool::new(false)),
            rate: cfg.as_ref().map_or(48_000, |c| c.sample_rate()),
            channels: cfg.as_ref().map_or(2, |c| c.channels()),
            device: device.and_then(|d| d.description().ok()).map(|d| d.name().to_string()),
        }
    }

    /// The device's name, for the status bar (`None` without a device).
    pub fn device(&self) -> Option<&str> {
        self.device.as_deref()
    }

    /// Start playing. `make_fill(sample_rate, channels)` builds the callback once the device's
    /// format is known. Returns the sample rate.
    pub fn start(&mut self, make_fill: impl FnOnce(u32, usize) -> Fill) -> Result<u32, String> {
        self.stop();
        let dev = cpal::default_host().default_output_device().ok_or("no audio output device")?;
        let cfg = dev.default_output_config().map_err(|e| e.to_string())?;
        let channels = usize::from(cfg.channels());
        if channels == 0 || cfg.sample_rate() == 0 {
            return Err("the output device reported no channels or no sample rate".into());
        }
        let config: cpal::StreamConfig = cfg.config();
        self.rate = cfg.sample_rate();
        self.channels = cfg.channels();
        self.played.store(0, Ordering::SeqCst);
        self.failed.store(false, Ordering::SeqCst);
        let fill = make_fill(self.rate, channels);
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => {
                let (played, failed) = (self.played.clone(), self.failed.clone());
                let mut fill = fill;
                dev.build_output_stream(
                    config,
                    move |buf: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        fill(buf, channels);
                        played.fetch_add((buf.len() / channels) as u64, Ordering::SeqCst);
                    },
                    move |e| stream_error(&failed, &e),
                    None,
                )
            }
            cpal::SampleFormat::I8 => self.converted::<i8>(&dev, config, fill),
            cpal::SampleFormat::I16 => self.converted::<i16>(&dev, config, fill),
            cpal::SampleFormat::I24 => self.converted::<cpal::I24>(&dev, config, fill),
            cpal::SampleFormat::I32 => self.converted::<i32>(&dev, config, fill),
            cpal::SampleFormat::I64 => self.converted::<i64>(&dev, config, fill),
            cpal::SampleFormat::U8 => self.converted::<u8>(&dev, config, fill),
            cpal::SampleFormat::U16 => self.converted::<u16>(&dev, config, fill),
            cpal::SampleFormat::U32 => self.converted::<u32>(&dev, config, fill),
            cpal::SampleFormat::U64 => self.converted::<u64>(&dev, config, fill),
            cpal::SampleFormat::F64 => self.converted::<f64>(&dev, config, fill),
            other => return Err(format!("unsupported sample format {other:?}")),
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        self.stream = Some(stream);
        Ok(self.rate)
    }

    /// A stream in an integer or f64 format: mixed as f32, converted per callback.
    fn converted<T>(
        &self,
        dev: &cpal::Device,
        config: cpal::StreamConfig,
        mut fill: Fill,
    ) -> Result<cpal::Stream, cpal::Error>
    where
        T: cpal::SizedSample + cpal::FromSample<f32>,
    {
        let channels = usize::from(config.channels);
        let (played, failed) = (self.played.clone(), self.failed.clone());
        let error_flag = failed.clone();
        let mut scratch = Vec::new();
        dev.build_output_stream(
            config,
            move |buf: &mut [T], _: &cpal::OutputCallbackInfo| {
                // A pathological driver buffer must not panic the audio thread.
                if buf.len() > 1_048_576 || scratch.try_reserve(buf.len().saturating_sub(scratch.len())).is_err() {
                    buf.fill(T::EQUILIBRIUM);
                    failed.store(true, Ordering::SeqCst);
                    return;
                }
                scratch.resize(buf.len(), 0.0);
                fill(&mut scratch, channels);
                convert_samples(&scratch, buf);
                played.fetch_add((buf.len() / channels) as u64, Ordering::SeqCst);
            },
            move |e| stream_error(&error_flag, &e),
            None,
        )
    }

    pub fn stop(&mut self) {
        self.stream = None;
    }

    /// The device's sample rate (the last stream's, or the default format's before any).
    pub fn sample_rate(&self) -> u32 {
        self.rate
    }

    pub fn channels(&self) -> usize {
        usize::from(self.channels)
    }

    /// Sample frames played since [`AudioOut::start`]: the playback clock. `None` when not
    /// playing or after a stream error, so the wall clock takes over.
    pub fn played_frames(&self) -> Option<u64> {
        self.stream.as_ref().filter(|_| !self.failed.load(Ordering::SeqCst)).map(|_| self.played.load(Ordering::SeqCst))
    }
}

fn stream_error(failed: &AtomicBool, e: &cpal::Error) {
    if !failed.swap(true, Ordering::SeqCst) {
        eprintln!("resolution: audio stream error: {e}");
    }
}

fn convert_samples<T: cpal::Sample + cpal::FromSample<f32>>(input: &[f32], output: &mut [T]) {
    // Integer formats represent [-1, 1): 24-bit cannot hold +1 exactly.
    let upper = f32::from_bits(1.0_f32.to_bits() - 1);
    for (sample, converted) in input.iter().zip(output) {
        *converted = T::from_sample(if sample.is_finite() { sample.clamp(-1.0, upper) } else { 0.0 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpal::Sample;

    #[test]
    fn integer_conversion_clips_and_preserves_interleaved_channels() {
        let mut signed = [0_i16; 8];
        convert_samples(&[-2.0, 2.0, -0.5, 0.5, 0.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY], &mut signed);
        assert_eq!(signed, [i16::MIN, i16::MAX, -16_384, 16_384, 0, 0, 0, 0]);
        let mut unsigned = [0_u16; 3];
        convert_samples(&[-1.0, 0.0, 1.0], &mut unsigned);
        assert_eq!(unsigned, [0, 32_768, u16::MAX]);
    }

    #[test]
    fn full_scale_24_bit_samples_remain_representable() {
        let mut samples = [cpal::I24::EQUILIBRIUM; 3];
        convert_samples(&[-1.0, 0.0, 1.0], &mut samples);
        assert_eq!(samples.map(|s| s.inner()), [-8_388_608, 0, 8_388_607]);
    }

    #[test]
    fn a_failed_stream_gives_up_the_clock() {
        let failed = AtomicBool::new(false);
        stream_error(&failed, &cpal::Error::from(cpal::ErrorKind::DeviceNotAvailable));
        assert!(failed.load(Ordering::SeqCst));
        assert_eq!(AudioOut::new().played_frames(), None, "not playing: no clock");
    }
}
