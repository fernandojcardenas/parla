# Contributing to Parla

Thanks for your interest in Parla. This is a small, focused app: local,
private, type-anywhere dictation for macOS. Contributions that keep it simple
and keep everything on-device are welcome.

## Getting set up

See the "Run locally" section of the [README](README.md). In short: download
the model with `./scripts/fetch-model.sh`, then `npm install` and
`npm run tauri dev`.

## Before you open a pull request

- Format and lint the Rust code:
  ```
  cd src-tauri
  cargo fmt
  cargo clippy
  ```
- Run the tests:
  ```
  cargo test --release
  ```
- Build the frontend so the TypeScript still type-checks:
  ```
  npm run build
  ```

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/): `feat:`,
`fix:`, `docs:`, `refactor:`, `test:`, `chore:`. Keep each commit focused.

## Ground rules

- Keep Parla offline. Do not add network calls, accounts, or telemetry to the
  running app.
- Never commit secrets, API keys, or personal data. The speech model is
  downloaded at build time, not stored in git.
