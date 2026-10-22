# Architecture

Parla is a Tauri 2 desktop app: a Rust backend does the audio and speech work,
and a small React window shows status and settings.

## Flow of a dictation

1. **Shortcut** — `tauri-plugin-global-shortcut` fires on press and release of
   the hold-to-talk key (`lib.rs`).
2. **Capture** — on press, `audio.rs` opens a `cpal` input stream on its own
   thread, mixes to mono and records into a buffer.
3. **Stop** — on release, the buffer is resampled to 16 kHz (what Whisper
   expects) and handed back.
4. **Transcribe** — `transcribe.rs` runs whisper.cpp (`whisper-rs`, Metal GPU)
   off the shortcut thread. It skips silence, pads very short clips, boosts
   quiet audio, and passes the custom vocabulary as Whisper's initial prompt.
5. **Clean up** — `cleanup.rs` removes filler words and non-speech noise tags
   with simple rules (no second model). If the optional AI cleanup setting is
   on, `ai_cleanup.rs` then sends that text (never audio) to the Anthropic
   Messages API with a 10-second timeout; on any failure (no key, network,
   HTTP error, refusal, empty reply) the rule-based text is used.
6. **Output** — `output.rs` copies the text to the clipboard, presses ⌘V on the
   main thread via `enigo`, and restores the previous clipboard. If macOS has
   not granted Accessibility, it leaves the text on the clipboard and reports
   how to enable pasting.

## Modules

| File | Responsibility |
| --- | --- |
| `lib.rs` | App setup, state, shortcut registration, status events, shutdown |
| `audio.rs` | Microphone capture thread and resampling |
| `transcribe.rs` | whisper.cpp model loading and transcription |
| `cleanup.rs` | Rule-based filler and noise removal |
| `ai_cleanup.rs` | Optional Claude cleanup, Keychain API key, frontmost app name |
| `output.rs` | Clipboard paste, clipboard restore, Accessibility check |
| `settings.rs` | Load and save user settings as JSON |
| `log.rs` | Log to the terminal and `~/Library/Logs/Parla/parla.log` |

## Model

The model (`ggml-large-v3-turbo-q5_0.bin`, ~550 MB) is fetched by
`scripts/fetch-model.sh` and bundled into the app as a resource at build time.
It is intentionally not stored in git.

## Shutdown

whisper.cpp holds GPU resources and has C++ static destructors that can abort on
exit. On the Tauri `Exit` event Parla unloads the model while the GPU is still
available and then calls `_exit(0)` to skip those destructors cleanly.
