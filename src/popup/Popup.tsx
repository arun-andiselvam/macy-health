import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Reminder = {
  id: string;
  kind: string;
  emoji: string;
  title: string;
  message: string;
};

type Action = "done" | "snooze" | "skip" | "dismissed";

const AUTO_DISMISS_MS = 30_000;
const EXIT_MS = 220;

export default function Popup() {
  const [reminder, setReminder] = useState<Reminder | null>(null);
  const [visible, setVisible] = useState(false);
  const [showCount, setShowCount] = useState(0);
  const currentId = useRef<string | null>(null);
  const closing = useRef(false);

  const show = useCallback((next: Reminder) => {
    // The on-load fetch and the show event can both deliver the same reminder.
    if (currentId.current === next.id && !closing.current) return;
    currentId.current = next.id;
    closing.current = false;
    setReminder(next);
    setShowCount((n) => n + 1);
    setVisible(true);
  }, []);

  useEffect(() => {
    const unlisten = listen<Reminder>("reminder:show", (e) => show(e.payload));
    invoke<Reminder | null>("popup_current").then((r) => r && show(r));
    return () => {
      unlisten.then((off) => off());
    };
  }, [show]);

  const act = useCallback(
    (action: Action) => {
      if (!reminder || closing.current) return;
      closing.current = true;
      setVisible(false);
      window.setTimeout(() => {
        invoke("popup_action", { id: reminder.id, action });
      }, EXIT_MS);
    },
    [reminder],
  );

  if (!reminder) return null;

  return (
    <div className="stage">
      <div
        key={showCount}
        className={`card ${visible ? "is-in" : "is-out"}`}
        data-kind={reminder.kind}
        role="alertdialog"
        aria-labelledby="popup-title"
        aria-describedby="popup-message"
      >
        <div className="icon" aria-hidden="true">
          <span>{reminder.emoji}</span>
        </div>

        <div className="body">
          <div id="popup-title" className="title">
            {reminder.title}
          </div>
          <div id="popup-message" className="message">
            {reminder.message}
          </div>
          <div className="actions">
            <button className="btn primary" onClick={() => act("done")}>
              Done
            </button>
            <button className="btn" onClick={() => act("snooze")}>
              Snooze 5m
            </button>
            <button className="btn ghost" onClick={() => act("skip")}>
              Skip
            </button>
          </div>
        </div>

        <div className="countdown" aria-hidden="true">
          <div
            className="countdown-bar"
            style={{ animationDuration: `${AUTO_DISMISS_MS}ms` }}
            onAnimationEnd={() => act("dismissed")}
          />
        </div>
      </div>
    </div>
  );
}
