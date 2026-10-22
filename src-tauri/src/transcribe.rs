//! On-device speech-to-text with whisper.cpp.

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::audio::TARGET_RATE;

/// File name of the model bundled with the app.
pub const MODEL_FILE: &str = "ggml-large-v3-turbo-q5_0.bin";

/// Recordings shorter than this are treated as accidental taps.
const MIN_SECONDS: f32 = 0.3;
/// If the loudest moment is below this, the recording is treated as silence.
/// Whisper tends to invent text ("Thank you.") when fed silence, so we skip it.
const SILENCE_PEAK: f32 = 0.003;
/// Quiet recordings are boosted so their loudest moment reaches this level.
const TARGET_PEAK: f32 = 0.9;
/// Never boost more than this, so background hiss doesn't become a roar.
const MAX_GAIN: f32 = 60.0;

pub struct Transcriber {
    ctx: WhisperContext,
}

impl Transcriber {
    pub fn load(model_path: &Path) -> Result<Self> {
        let path = model_path
            .to_str()
            .ok_or_else(|| anyhow!("Model path is not valid text"))?;
        let mut params = WhisperContextParameters::default();
        params.use_gpu(true);
        let ctx = WhisperContext::new_with_params(path, params)
            .with_context(|| format!("Could not load the speech model at {path}"))?;
        Ok(Self { ctx })
    }

    /// Turns 16 kHz mono audio into text. `vocabulary` is a list of words
    /// (names, jargon) Whisper should prefer when it hears something close.
    pub fn transcribe(&self, samples: &[f32], vocabulary: &[String]) -> Result<String> {
        if !has_speech(samples) {
            return Ok(String::new());
        }

        let mut state = self.ctx.create_state().context("Could not start transcription")?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("en"));
        params.set_n_threads(num_threads());
        params.set_no_context(true);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        let prompt = vocabulary_prompt(vocabulary);
        if !prompt.is_empty() {
            params.set_initial_prompt(&prompt);
        }

        // Whisper needs at least one second of audio; pad short clips with silence.
        let mut audio = normalize(samples);
        let min_len = TARGET_RATE as usize + TARGET_RATE as usize / 10;
        if audio.len() < min_len {
            audio.resize(min_len, 0.0);
        }

        state.full(params, &audio).context("Transcription failed")?;

        let mut text = String::new();
        for segment in state.as_iter() {
            if let Ok(s) = segment.to_str_lossy() {
                text.push_str(&s);
            }
        }
        Ok(text)
    }
}

fn num_threads() -> i32 {
    std::thread::available_parallelism()
        .map(|n| n.get().min(8) as i32)
        .unwrap_or(4)
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0f32, |m, s| m.max(s.abs()))
}

fn has_speech(samples: &[f32]) -> bool {
    (samples.len() as f32) >= MIN_SECONDS * TARGET_RATE as f32 && peak(samples) >= SILENCE_PEAK
}

/// Brings quiet recordings (a mic far away, low input gain) up to a normal level.
fn normalize(samples: &[f32]) -> Vec<f32> {
    let p = peak(samples);
    if p <= 0.0 || p >= TARGET_PEAK {
        return samples.to_vec();
    }
    let gain = (TARGET_PEAK / p).min(MAX_GAIN);
    samples.iter().map(|s| s * gain).collect()
}

/// Whisper's "initial prompt" nudges spelling toward words it has seen.
fn vocabulary_prompt(words: &[String]) -> String {
    let words: Vec<&str> = words
        .iter()
        .map(|w| w.trim())
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        String::new()
    } else {
        format!("Vocabulary: {}.", words.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_skipped() {
        assert!(!has_speech(&vec![0.0; TARGET_RATE as usize * 2]));
        assert!(!has_speech(&vec![0.5; 100]));
    }

    #[test]
    fn quiet_audio_is_boosted() {
        let out = normalize(&[0.05, -0.09]);
        assert!((peak(&out) - TARGET_PEAK).abs() < 1e-4);
        // Loud audio is left alone.
        assert_eq!(normalize(&[0.95]), vec![0.95]);
    }

    #[test]
    fn prompt_lists_words() {
        let words = vec!["Cardenas".to_string(), " ".to_string(), "WGU".to_string()];
        assert_eq!(vocabulary_prompt(&words), "Vocabulary: Cardenas, WGU.");
        assert_eq!(vocabulary_prompt(&[]), "");
    }
}
