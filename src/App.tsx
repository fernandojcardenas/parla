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
  ai_cleanup: boolean;
  claude_model: string;
  anthropic_workspace_id: string;
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
  const [hasKey, setHasKey] = useState(false);
  const [keyInput, setKeyInput] = useState("");
  const [keyMsg, setKeyMsg] = useState("");

  useEffect(() => {
    invoke<Status>("get_status").then(setStatus);
    invoke<Settings>("get_settings").then((s) => {
      setSettings(s);
      setVocabText(s.vocabulary.join("\n"));
    });
    invoke<string[]>("list_microphones").then(setMics);
    invoke<boolean>("has_api_key").then(setHasKey);
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

  /** Saves the key to the macOS Keychain; an empty key removes it. */
  async function saveKey(key: string) {
    try {
      await invoke("set_api_key", { key });
      const saved = await invoke<boolean>("has_api_key");
      setHasKey(saved);
      setKeyInput("");
      setKeyMsg(saved ? "Key saved to Keychain" : "Key removed");
    } catch (e) {
      setKeyMsg(String(e));
    }
    setTimeout(() => setKeyMsg(""), 4000);
  }

  const shortcut = settings?.shortcut ?? "Alt+Space";
  const online = settings?.ai_cleanup ?? false;

  return (
    <main className="app">
      <header>
        <h1>Parla</h1>
        <p className="tagline">
          {online
            ? "Free dictation. Speech stays on this Mac; AI cleanup sends the text to Anthropic."
            : "Free, private dictation. Everything stays on this Mac."}
        </p>
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

          <label className="check">
            <input
              type="checkbox"
              checked={settings.ai_cleanup}
              onChange={(e) => setSettings({ ...settings, ai_cleanup: e.target.checked })}
            />
            AI cleanup with Claude (sends the text, never audio, to Anthropic)
          </label>

          {settings.ai_cleanup && (
            <div className="ai-settings">
              <p className="hint">
                Fixes self-corrections ("Tuesday, no wait, Wednesday"), formats spoken lists and
                tidies punctuation. If Claude can't be reached, Parla pastes the offline text.
              </p>

              <label>
                Anthropic API key {hasKey ? "(saved in Keychain)" : "(not set)"}
                <div className="key-row">
                  <input
                    type="password"
                    value={keyInput}
                    onChange={(e) => setKeyInput(e.target.value)}
                    placeholder={hasKey ? "Enter a new key to replace it" : "sk-ant-…"}
                    autoComplete="off"
                    spellCheck={false}
                  />
                  <button onClick={() => saveKey(keyInput)} disabled={!keyInput.trim()}>
                    Save key
                  </button>
                  {hasKey && (
                    <button className="secondary" onClick={() => saveKey("")}>
                      Remove
                    </button>
                  )}
                </div>
                <span className="save-msg">{keyMsg}</span>
              </label>

              <label>
                Claude model
                <input
                  value={settings.claude_model}
                  onChange={(e) => setSettings({ ...settings, claude_model: e.target.value })}
                  placeholder="claude-haiku-5-5"
                  spellCheck={false}
                />
              </label>

              <label>
                Workspace ID (only for organization-wide keys)
                <input
                  value={settings.anthropic_workspace_id}
                  onChange={(e) => setSettings({ ...settings, anthropic_workspace_id: e.target.value })}
                  placeholder="Optional"
                  spellCheck={false}
                />
              </label>
            </div>
          )}

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
