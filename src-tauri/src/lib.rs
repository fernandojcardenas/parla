mod audio;
#[macro_use]
mod log;
mod ai_cleanup;
mod cleanup;
mod output;
mod settings;
mod transcribe;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Instant;

use serde::Serialize;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use audio::Recorder;
use settings::Settings;
use transcribe::{Transcriber, MODEL_FILE};

/// What Parla is doing right now. Sent to the window as the "status" event.
#[derive(Clone, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
enum Status {
    Loading,
    Idle,
    Recording,
    Transcribing,
    Done { text: String, millis: u128 },
    Error { message: String },
}

struct AppState {
    recorder: Recorder,
    transcriber: Arc<RwLock<Option<Transcriber>>>,
    settings: RwLock<Settings>,
    settings_path: PathBuf,
    recording: Mutex<bool>,
    last_status: Mutex<Status>,
    /// Set when the dictation shortcut could not be registered.
    shortcut_error: Mutex<Option<String>>,
}

fn set_status(app: &AppHandle, status: Status) {
    match &status {
        Status::Error { message } => plog!("error: {message}"),
        Status::Idle => plog!("ready"),
        _ => {}
    }
    if let Some(state) = app.try_state::<AppState>() {
        *state.last_status.lock().unwrap() = status.clone();
    }
    let _ = app.emit("status", status);
}

fn on_press(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut recording = state.recording.lock().unwrap();
    if *recording {
        return;
    }
    if state.transcriber.read().unwrap().is_none() {
        set_status(app, Status::Error { message: "The speech model is still loading. Try again in a moment.".into() });
        return;
    }
    let mic = state.settings.read().unwrap().microphone.clone();
    match state.recorder.start(mic) {
        Ok(()) => {
            *recording = true;
            set_status(app, Status::Recording);
        }
        Err(e) => set_status(app, Status::Error { message: format!("{e:#}") }),
    }
}

fn on_release(app: &AppHandle) {
    let state = app.state::<AppState>();
    {
        let mut recording = state.recording.lock().unwrap();
        if !*recording {
            return;
        }
        *recording = false;
    }
    let samples = state.recorder.stop();
    let peak = samples.iter().fold(0f32, |m, s| m.max(s.abs()));
    plog!(
        "recorded {:.1}s, peak level {:.4}",
        samples.len() as f32 / audio::TARGET_RATE as f32,
        peak
    );
    set_status(app, Status::Transcribing);

    // Transcribe off the shortcut thread so the app stays responsive.
    let app = app.clone();
    thread::spawn(move || {
        let state = app.state::<AppState>();
        let settings = state.settings.read().unwrap().clone();
        let started = Instant::now();
        let raw = {
            let guard = state.transcriber.read().unwrap();
            match guard.as_ref() {
                Some(t) => t.transcribe(&samples, &settings.vocabulary),
                None => Ok(String::new()),
            }
        };
        match raw {
            Ok(raw) => {
                let mut text = cleanup::clean(&raw, settings.remove_fillers);
                plog!("transcribed in {:?}: {text:?}", started.elapsed());
                if settings.ai_cleanup && !text.is_empty() {
                    let ai_started = Instant::now();
                    let api_key = ai_cleanup::load_api_key();
                    let target_app = ai_cleanup::frontmost_app();
                    text = ai_cleanup::refine(
                        &text,
                        &ai_cleanup::Options {
                            api_key: api_key.as_deref(),
                            model: &settings.claude_model,
                            workspace_id: &settings.anthropic_workspace_id,
                            vocabulary: &settings.vocabulary,
                            target_app: target_app.as_deref(),
                        },
                    );
                    plog!("AI cleanup in {:?}: {text:?}", ai_started.elapsed());
                }
                if !text.is_empty() {
                    let trusted = output::can_control_keyboard();
                    plog!("pasting (accessibility granted: {trusted})");
                    if !trusted {
                        // Can't press ⌘V for the user, so leave the words on the
                        // clipboard rather than losing them.
                        let copied = output::copy(&text).is_ok();
                        let message = if copied {
                            "Copied to your clipboard. Press ⌘V to paste. To paste automatically, allow Parla in System Settings → Privacy & Security → Accessibility, then reopen Parla."
                        } else {
                            "Parla needs Accessibility permission to paste. Allow it in System Settings → Privacy & Security → Accessibility, then reopen Parla."
                        };
                        set_status(&app, Status::Error { message: message.into() });
                        return;
                    }
                    if let Err(e) = output::paste(&text, settings.restore_clipboard, |job| run_on_main(&app, job)) {
                        set_status(&app, Status::Error { message: format!("{e:#}") });
                        return;
                    }
                }
                set_status(&app, Status::Done { text, millis: started.elapsed().as_millis() });
            }
            Err(e) => {
                plog!("transcription error: {e:#}");
                set_status(&app, Status::Error { message: format!("{e:#}") });
            }
        }
    });
}

/// Runs `job` on the main thread and waits for its result.
fn run_on_main(
    app: &AppHandle,
    job: Box<dyn FnOnce() -> anyhow::Result<()> + Send>,
) -> anyhow::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(job());
    })
    .map_err(|e| anyhow::anyhow!("Could not reach the main thread: {e}"))?;
    rx.recv()
        .map_err(|_| anyhow::anyhow!("The paste step did not finish"))?
}

/// Registers the hold-to-talk shortcut, replacing any previous one.
fn register_shortcut(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let result = gs
        .on_shortcut(shortcut, |app, _shortcut, event| match event.state {
            ShortcutState::Pressed => on_press(app),
            ShortcutState::Released => on_release(app),
        })
        .map_err(|e| {
            format!("Could not use the shortcut \"{shortcut}\" ({e}). Another app may already be using it.")
        });
    if let Some(state) = app.try_state::<AppState>() {
        *state.shortcut_error.lock().unwrap() = result.clone().err();
        if result.is_ok() && state.transcriber.read().unwrap().is_some() {
            set_status(app, Status::Idle);
        }
    }
    result
}

fn model_path(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(p) = std::env::var("PARLA_MODEL") {
        return Some(PathBuf::from(p));
    }
    app.path()
        .resolve(format!("resources/models/{MODEL_FILE}"), BaseDirectory::Resource)
        .ok()
        .filter(|p| p.exists())
}

fn load_model_in_background(app: AppHandle) {
    thread::spawn(move || {
        set_status(&app, Status::Loading);
        let Some(path) = model_path(&app) else {
            set_status(&app, Status::Error { message: format!("Speech model {MODEL_FILE} is missing from the app.") });
            return;
        };
        match Transcriber::load(&path) {
            Ok(t) => {
                let state = app.state::<AppState>();
                *state.transcriber.write().unwrap() = Some(t);
                // Don't hide a shortcut problem behind a "ready" message.
                match state.shortcut_error.lock().unwrap().clone() {
                    Some(message) => set_status(&app, Status::Error { message }),
                    None => set_status(&app, Status::Idle),
                }
            }
            Err(e) => {
                plog!("transcription error: {e:#}");
                set_status(&app, Status::Error { message: format!("{e:#}") });
            }
        }
    });
}

// ---- Commands the window can call ----

#[tauri::command]
fn get_status(state: State<AppState>) -> Status {
    state.last_status.lock().unwrap().clone()
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.read().unwrap().clone()
}

#[tauri::command]
fn save_settings(app: AppHandle, state: State<AppState>, settings: Settings) -> Result<(), String> {
    let old_shortcut = state.settings.read().unwrap().shortcut.clone();
    if settings.shortcut != old_shortcut {
        if let Err(e) = register_shortcut(&app, &settings.shortcut) {
            // Keep the old shortcut working if the new one is rejected.
            let _ = register_shortcut(&app, &old_shortcut);
            return Err(e);
        }
    }
    settings.save(&state.settings_path).map_err(|e| e.to_string())?;
    *state.settings.write().unwrap() = settings;
    Ok(())
}

/// Whether an Anthropic API key is saved. The key itself never leaves Rust.
#[tauri::command]
fn has_api_key() -> bool {
    ai_cleanup::load_api_key().is_some()
}

/// Saves the Anthropic API key to the Keychain, or removes it when empty.
#[tauri::command]
fn set_api_key(key: String) -> Result<(), String> {
    ai_cleanup::store_api_key(&key)
}

#[tauri::command]
fn list_microphones() -> Vec<String> {
    audio::input_device_names()
}

#[tauri::command]
fn has_accessibility() -> bool {
    output::can_control_keyboard()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let handle = app.handle().clone();
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let settings = Settings::load(&settings_path);
            let shortcut = settings.shortcut.clone();
            plog!(
                "starting Parla {} (accessibility granted: {})",
                env!("CARGO_PKG_VERSION"),
                output::can_control_keyboard()
            );

            app.manage(AppState {
                recorder: Recorder::spawn(),
                transcriber: Arc::new(RwLock::new(None)),
                settings: RwLock::new(settings),
                settings_path,
                recording: Mutex::new(false),
                last_status: Mutex::new(Status::Loading),
                shortcut_error: Mutex::new(None),
            });

            // Without Accessibility, Parla can't paste. Creating the keyboard
            // controller makes macOS show its permission prompt now, at launch.
            if !output::can_control_keyboard() {
                let _ = enigo::Enigo::new(&enigo::Settings::default());
            }

            load_model_in_background(handle.clone());
            if let Err(message) = register_shortcut(&handle, &shortcut) {
                set_status(&handle, Status::Error { message });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_settings,
            save_settings,
            list_microphones,
            has_accessibility,
            has_api_key,
            set_api_key
        ])
        .build(tauri::generate_context!())
        .expect("error while starting Parla")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                // Unload the speech model while the GPU is still available.
                // Leaving it to the process's own shutdown makes Metal abort.
                if let Some(state) = app.try_state::<AppState>() {
                    state.transcriber.write().unwrap().take();
                }
                plog!("quit");
                // Skip C++ static destructors in whisper.cpp; they also crash on exit.
                unsafe extern "C" {
                    fn _exit(code: i32) -> !;
                }
                unsafe { _exit(0) }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end check of the speech pipeline on a known recording.
    /// Needs the model: run scripts/fetch-model.sh first.
    #[test]
    fn transcribes_sample_recording() {
        let model = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/models").join(MODEL_FILE);
        if !model.exists() {
            eprintln!("skipping: model not downloaded");
            return;
        }
        let wav = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jfk.wav");
        let mut reader = hound::WavReader::open(wav).unwrap();
        let spec = reader.spec();
        let samples: Vec<f32> = reader.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect();
        let samples = audio::resample(&samples, spec.sample_rate, audio::TARGET_RATE);

        let t = Transcriber::load(&model).unwrap();
        let started = Instant::now();
        let raw = t.transcribe(&samples, &[]).unwrap();
        let text = cleanup::clean(&raw, true);
        eprintln!("transcribed in {:?}: {text}", started.elapsed());
        assert!(text.to_lowercase().contains("ask not what your country can do for you"));
    }
}
