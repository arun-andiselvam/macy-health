import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Reminder } from "./types";
import { ReminderRow } from "./settings/ReminderRow";
import { ReminderEditor } from "./settings/ReminderEditor";
import "./App.css";

const NEW_REMINDER: Reminder = {
  id: "",
  kind: "custom",
  emoji: "🧘",
  title: "",
  message: "",
  intervalMinutes: 30,
  enabled: true,
  sound: true,
  activeStart: "08:00",
  activeEnd: "20:00",
  days: [1, 2, 3, 4, 5, 6, 7],
};

const errorText = (err: unknown) => (typeof err === "string" ? err : "Something went wrong. Try again.");

function App() {
  const [reminders, setReminders] = useState<Reminder[]>([]);
  const [draft, setDraft] = useState<Reminder | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    invoke<Reminder[]>("get_reminders")
      .then(setReminders)
      .catch((err) => setLoadError(errorText(err)));
    const unlisten = listen<Reminder[]>("reminders:changed", (e) => setReminders(e.payload));
    return () => {
      unlisten.then((off) => off());
    };
  }, []);

  const edit = useCallback((reminder: Reminder | null) => {
    setDraft(reminder && { ...reminder, days: [...reminder.days] });
    setError(null);
  }, []);

  const save = async () => {
    if (!draft) return;
    setSaving(true);
    setError(null);
    try {
      // The switch may have changed while editing; keep its current value.
      const current = reminders.find((r) => r.id === draft.id);
      const payload = { ...draft, enabled: current?.enabled ?? true };
      setReminders(await invoke<Reminder[]>("save_reminder", { reminder: payload }));
      setDraft(null);
    } catch (err) {
      setError(errorText(err));
    } finally {
      setSaving(false);
    }
  };

  const remove = async () => {
    if (!draft) return;
    try {
      setReminders(await invoke<Reminder[]>("delete_reminder", { id: draft.id }));
      setDraft(null);
    } catch (err) {
      setError(errorText(err));
    }
  };

  const preview = async () => {
    if (!draft) return;
    try {
      await invoke("preview_reminder", { reminder: draft });
      setError(null);
    } catch (err) {
      setError(errorText(err));
    }
  };

  const setEnabled = async (id: string, enabled: boolean) => {
    setReminders((list) => list.map((r) => (r.id === id ? { ...r, enabled } : r)));
    try {
      setReminders(await invoke<Reminder[]>("set_reminder_enabled", { id, enabled }));
    } catch (err) {
      setLoadError(errorText(err));
    }
  };

  const creating = draft?.id === "";
  // While editing, the row shows the draft so its schedule and timeline update live.
  const rows = [...(creating && draft ? [draft] : []), ...reminders].map((r) =>
    draft && r.id === draft.id ? { ...draft, enabled: r.enabled } : r,
  );

  return (
    <main className="page">
      <header className="page-header">
        <div>
          <h1>Reminders</h1>
          <p className="muted">
            Each reminder pops up in the top-right corner of your screen, only during its active hours.
          </p>
        </div>
        <button type="button" className="button primary" onClick={() => edit(NEW_REMINDER)} disabled={creating}>
          Add reminder
        </button>
      </header>

      {loadError && (
        <p className="error" role="alert">
          {loadError}
        </p>
      )}

      <ul className="group">
        {rows.map((reminder) => {
          const expanded = draft?.id === reminder.id;
          return (
            <ReminderRow
              key={reminder.id || "new"}
              reminder={reminder}
              expanded={expanded}
              onToggleExpanded={() => edit(expanded ? null : reminder)}
              onToggleEnabled={(enabled) => setEnabled(reminder.id, enabled)}
            >
              {expanded && draft && (
                <ReminderEditor
                  draft={draft}
                  onChange={setDraft}
                  onSave={save}
                  onCancel={() => edit(null)}
                  onDelete={draft.kind === "custom" && draft.id !== "" ? remove : undefined}
                  onPreview={preview}
                  saving={saving}
                  error={error}
                />
              )}
            </ReminderRow>
          );
        })}
      </ul>

      {rows.length === 0 && !loadError && (
        <p className="empty muted">No reminders yet. Add one to get started.</p>
      )}
    </main>
  );
}

export default App;
