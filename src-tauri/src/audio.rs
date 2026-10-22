//! Microphone capture.
//!
//! The audio stream lives on its own thread because a cpal stream must stay on
//! the thread that created it. The rest of the app talks to that thread
//! through a small command channel: start recording, stop and hand back the
//! samples.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SizedSample};

/// Whisper expects 16 kHz mono audio.
pub const TARGET_RATE: u32 = 16_000;

enum Command {
    Start(Option<String>, Sender<Result<()>>),
    Stop(Sender<Vec<f32>>),
}

/// Handle to the recording thread. Cheap to clone.
#[derive(Clone)]
pub struct Recorder {
    tx: Sender<Command>,
}

impl Recorder {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        thread::Builder::new()
            .name("parla-audio".into())
            .spawn(move || audio_thread(rx))
            .expect("failed to start audio thread");
        Self { tx }
    }

    /// Start recording from the named input device, or the system default.
    pub fn start(&self, device: Option<String>) -> Result<()> {
        let (reply, wait) = mpsc::channel();
        self.tx
            .send(Command::Start(device, reply))
            .map_err(|_| anyhow!("audio thread stopped"))?;
        wait.recv().map_err(|_| anyhow!("audio thread stopped"))?
    }

    /// Stop recording and return the audio as 16 kHz mono samples.
    pub fn stop(&self) -> Vec<f32> {
        let (reply, wait) = mpsc::channel();
        if self.tx.send(Command::Stop(reply)).is_err() {
            return Vec::new();
        }
        wait.recv().unwrap_or_default()
    }
}

/// Names of the microphones the system can see.
pub fn input_device_names() -> Vec<String> {
    let host = cpal::default_host();
    match host.input_devices() {
        Ok(devices) => devices.map(|d| d.to_string()).collect(),
        Err(_) => Vec::new(),
    }
}

struct Active {
    _stream: cpal::Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
    rate: u32,
}

fn audio_thread(rx: Receiver<Command>) {
    let mut active: Option<Active> = None;
    for cmd in rx {
        match cmd {
            Command::Start(device, reply) => {
                // Starting while already recording just keeps the current take.
                if active.is_some() {
                    let _ = reply.send(Ok(()));
                    continue;
                }
                match open_stream(device.as_deref()) {
                    Ok(a) => {
                        active = Some(a);
                        let _ = reply.send(Ok(()));
                    }
                    Err(e) => {
                        let _ = reply.send(Err(e));
                    }
                }
            }
            Command::Stop(reply) => {
                let samples = match active.take() {
                    Some(a) => {
                        // Dropping the stream stops capture.
                        let rate = a.rate;
                        let buffer = a.buffer.clone();
                        drop(a);
                        let raw = std::mem::take(&mut *buffer.lock().unwrap());
                        resample(&raw, rate, TARGET_RATE)
                    }
                    None => Vec::new(),
                };
                let _ = reply.send(samples);
            }
        }
    }
}

fn find_device(name: Option<&str>) -> Result<cpal::Device> {
    let host = cpal::default_host();
    if let Some(wanted) = name {
        if let Ok(mut devices) = host.input_devices() {
            if let Some(d) = devices.find(|d| d.to_string() == wanted) {
                return Ok(d);
            }
        }
        // The chosen mic is unplugged: fall back to the default instead of failing.
    }
    host.default_input_device()
        .ok_or_else(|| anyhow!("No microphone found"))
}

fn open_stream(name: Option<&str>) -> Result<Active> {
    let device = find_device(name)?;
    let supported = device
        .default_input_config()
        .context("Could not read the microphone's settings")?;
    let config = supported.config();
    let channels = config.channels as usize;
    let rate = config.sample_rate;
    let buffer = Arc::new(Mutex::new(Vec::<f32>::with_capacity(rate as usize * 30)));

    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config, channels, buffer.clone())?,
        SampleFormat::I16 => build::<i16>(&device, config, channels, buffer.clone())?,
        SampleFormat::I32 => build::<i32>(&device, config, channels, buffer.clone())?,
        SampleFormat::U16 => build::<u16>(&device, config, channels, buffer.clone())?,
        other => return Err(anyhow!("Unsupported microphone sample format: {other:?}")),
    };
    stream.play().context("Could not start the microphone")?;
    Ok(Active { _stream: stream, buffer, rate })
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: usize,
    buffer: Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream>
where
    T: SizedSample + Send + 'static,
    f32: cpal::FromSample<T>,
{
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _| {
            let mut buf = buffer.lock().unwrap();
            // Average all channels down to mono.
            for frame in data.chunks(channels.max(1)) {
                let sum: f32 = frame
                    .iter()
                    .map(|s| <f32 as cpal::FromSample<T>>::from_sample_(*s))
                    .sum();
                buf.push(sum / frame.len() as f32);
            }
        },
        |err| crate::log::write(&format!("microphone error: {err}")),
        None,
    )?;
    Ok(stream)
}

/// Converts audio to a new sample rate with linear interpolation.
/// Good enough for speech, which Whisper only needs at 16 kHz.
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let out_len = (input.len() as f64 / ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let idx = pos.floor() as usize;
        let frac = (pos - idx as f64) as f32;
        let a = input[idx];
        let b = *input.get(idx + 1).unwrap_or(&a);
        out.push(a + (b - a) * frac);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_halves_length() {
        let input: Vec<f32> = (0..32_000).map(|i| (i as f32).sin()).collect();
        let out = resample(&input, 32_000, 16_000);
        assert_eq!(out.len(), 16_000);
    }

    #[test]
    fn resample_same_rate_is_unchanged() {
        let input = vec![0.1, 0.2, 0.3];
        assert_eq!(resample(&input, 16_000, 16_000), input);
    }
}
