# Parla

**A free, local, type-anywhere voice dictation app.** Hold a hotkey, speak, and
your words are transcribed on-device and typed into whatever app you're using.
No cloud, no subscription, no account.

> Original application by Fernando Cardenas, MIT licensed. Built from scratch
> with Tauri + Rust. Work in progress.

## How it will work
1. Hold a global shortcut and speak.
2. Audio is captured locally and transcribed by an on-device speech model.
3. The text is typed at your cursor in any app.
4. Optional local AI cleanup (punctuation, filler-word removal) and a custom
   vocabulary.

## Status
Early scaffold. Milestones: (1) baseline app builds, (2) dictation pipeline,
(3) tray + settings UI, (4) local cleanup + vocabulary, (5) packaging.

## Tech & credits
Parla is original code. Like every dictation app, it relies on open-source
components for the hard AI/OS parts, used under their own licenses and credited:
- **Speech recognition:** OpenAI **Whisper** / NVIDIA **Parakeet** (open models),
  run locally (planned via whisper.cpp bindings).
- **App framework:** Tauri + Rust. Audio: cpal. Text input: enigo (planned).

## Build from source
Requires Rust (stable, edition 2024 -> Rust 1.85+), Node.js, and the Tauri
prerequisites for your OS (on macOS, Xcode Command Line Tools).
```
npm install
npm run tauri dev
```

## License
MIT - see [LICENSE](LICENSE).
