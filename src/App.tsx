import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import "./App.css";

type Status =
  | { state: "loading" }
  | { state: "idle" }
  | { state: "recording" }
  | { state: "transcribing" }
  | { state: "done"; text: string; millis: number }
  | { state: "error"; message: string };

type Settings = {
  shortcut: string;
  microphone: string | null;
  remove_fillers: boolean;
  restore_clipboard: boolean;
  vocabulary: string[];
};

const ACCESSIBILITY_URL =
  "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

/** "Alt+Space" -> "⌥ Space" so the shortcut reads like a Mac shortcut. */
function prettyShortcut(s: string) {
  return s
    .replace(/Alt\+?/gi, "⌥ ")
    .replace(/(Cmd|Super|Meta)\+?/gi, "⌘ ")
    .replace(/(Ctrl|Control)\+?/gi, "⌃ ")
    .replace(/Shift\+?/gi, "⇧ ")
    .trim();
}

function statusLine(status: Status, shortcut: string) {
  switch (status.state) {
    case "loading":
      return "Loading the speech model…";
    case "idle":
      return `Hold ${prettyShortcut(shortcut)} and speak`;
    case "recording":
      return "Listening…";
    case "transcribing":
      return "Transcribing…";
    case "done":
      return status.text ? `Done in ${(status.millis / 1000).toFixed(1)}s` : "Didn't catch any speech";
    case "error":
      return status.message;
  }
}

export default function App() {
  const [status, setStatus] = useState<Status>({ state: "loading" });
  const [settings, setSettings] = useState<Settings | null>(null);
  const [vocabText, setVocabText] = useState("");
  const [mics, setMics] = useState<string[]>([]);
  const [trusted, setTrusted] = useState(true);
  const [lastText, setLastText] = useState("");
  const [saveMsg, setSaveMsg] = useState("");

  useEffect(() => {
    invoke<Status>("get_status").then(setStatus);
    invoke<Settings>("get_settings").then((s) => {
      setSettings(s);
      setVocabText(s.vocabulary.join("\n"));
    });
    invoke<string[]>("list_microphones").then(setMics);
    const unlisten = listen<Status>("status", (e) => {
      setStatus(e.payload);
      if (e.payload.state === "done" && e.payload.text) setLastText(e.payload.text);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // Accessibility can be granted while the app is open, so keep checking.
  useEffect(() => {
    const check = () => invoke<boolean>("has_accessibility").then(setTrusted);
    check();
    const id = setInterval(check, 2000);
    return () => clearInterval(id);
  }, []);

  async function save() {
    if (!settings) return;
    const next = {
      ...settings,
      vocabulary: vocabText.split(/[\n,]/).map((w) => w.trim()).filter(Boolean),
    };
    try {
      await invoke("save_settings", { settings: next });
      setSettings(next);
      setSaveMsg("Saved");
    } catch (e) {
      setSaveMsg(String(e));
    }
    setTimeout(() => setSaveMsg(""), 4000);
  }

  const shortcut = settings?.shortcut ?? "Alt+Space";

  return (
    <main className="app">
      <header>
        <h1>Parla</h1>
        <p className="tagline">Free, private dictation. Everything stays on this Mac.</p>
      </header>

      <section className={`status status-${status.state}`} aria-live="polite">
        <span className="dot" />
        <span>{statusLine(status, shortcut)}</span>
      </section>

      {!trusted && (
        <section className="notice">
          <p>
            Parla needs <strong>Accessibility</strong> permission to type into other apps.
          </p>
          <button onClick={() => openUrl(ACCESSIBILITY_URL)}>Open Accessibility settings</button>
        </section>
      )}

      {lastText && (
        <section className="last">
          <h2>Last dictation</h2>
          <p>{lastText}</p>
        </section>
      )}

      {settings && (
        <section className="settings">
          <h2>Settings</h2>

          <label>
            Shortcut (hold to talk)
            <input
              value={settings.shortcut}
              onChange={(e) => setSettings({ ...settings, shortcut: e.target.value })}
              placeholder="Alt+Space"
            />
          </label>

          <label>
            Microphone
            <select
              value={settings.microphone ?? ""}
              onChange={(e) => setSettings({ ...settings, microphone: e.target.value || null })}
            >
              <option value="">System default</option>
              {mics.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
          </label>

          <label className="check">
            <input
              type="checkbox"
              checked={settings.remove_fillers}
              onChange={(e) => setSettings({ ...settings, remove_fillers: e.target.checked })}
            />
            Remove filler words (um, uh)
          </label>

          <label className="check">
            <input
              type="checkbox"
              checked={settings.restore_clipboard}
              onChange={(e) => setSettings({ ...settings, restore_clipboard: e.target.checked })}
            />
            Keep my clipboard unchanged
          </label>

          <label>
            Custom words (one per line)
            <textarea
              rows={5}
              value={vocabText}
              onChange={(e) => setVocabText(e.target.value)}
              placeholder={"Cardenas\nWGU\nParakeet"}
            />
          </label>

          <div className="save-row">
            <button onClick={save}>Save</button>
            <span className="save-msg">{saveMsg}</span>
          </div>
        </section>
      )}
    </main>
  );
}
