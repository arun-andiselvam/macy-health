import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Illustration } from "./illustrations";

type Reminder = {
  id: string;
  kind: string;
  emoji: string;
  title: string;
  message: string;
  sound: boolean;
};

type Action = "done" | "snooze" | "skip" | "dismissed";

const AUTO_DISMISS_MS = 30_000;
const EXIT_MS = 220;
const EYE_REST_SECONDS = 20;
const EYE_REST_CELEBRATE_MS = 1_600;

export default function Popup() {
  const [reminder, setReminder] = useState<Reminder | null>(null);
  const [visible, setVisible] = useState(false);
  const [showCount, setShowCount] = useState(0);
  const [secondsLeft, setSecondsLeft] = useState(EYE_REST_SECONDS);
  const [completed, setCompleted] = useState(false);
  const currentId = useRef<string | null>(null);
  const closing = useRef(false);

  const show = useCallback((next: Reminder) => {
    // The on-load fetch and the show event can both deliver the same reminder.
    if (currentId.current === next.id && !closing.current) return;
    currentId.current = next.id;
    closing.current = false;
    setReminder(next);
    setSecondsLeft(EYE_REST_SECONDS);
    setCompleted(false);
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

  const actRef = useRef(act);
  actRef.current = act;

  const isEyeRest = reminder?.kind === "eyes";
  const soundOn = reminder?.sound ?? false;

  // Eye rest: count down 20s, then chime and complete as Done.
  useEffect(() => {
    if (!isEyeRest || !visible) return;
    const startedAt = Date.now();
    let doneTimer: number | undefined;
    const tick = window.setInterval(() => {
      const elapsed = Math.floor((Date.now() - startedAt) / 1000);
      const left = Math.max(0, EYE_REST_SECONDS - elapsed);
      setSecondsLeft(left);
      if (left > 0) return;
      window.clearInterval(tick);
      setCompleted(true);
      if (soundOn) invoke("play_sound", { sound: "complete" });
      doneTimer = window.setTimeout(() => actRef.current("done"), EYE_REST_CELEBRATE_MS);
    }, 250);
    return () => {
      window.clearInterval(tick);
      window.clearTimeout(doneTimer);
    };
  }, [showCount, isEyeRest, visible, soundOn]);

  if (!reminder) return null;

  const title = isEyeRest && completed ? "Well done!" : reminder.title;
  const message = isEyeRest && completed ? "Your eyes thank you." : reminder.message;

  return (
    <div className="stage">
      <div
        key={showCount}
        className={`card ${visible ? "is-in" : "is-out"} ${completed ? "is-complete" : ""}`}
        data-kind={reminder.kind}
        role="alertdialog"
        aria-labelledby="popup-title"
        aria-describedby="popup-message"
      >
        <div className="icon">
          <Illustration
            kind={reminder.kind}
            emoji={reminder.emoji}
            countdownMs={EYE_REST_SECONDS * 1000}
          />
        </div>

        <div className="body">
          <div className="title-row">
            <div id="popup-title" className="title">
              {title}
            </div>
            {isEyeRest && !completed && (
              <div className="timer" aria-live="off">
                {secondsLeft}s
              </div>
            )}
          </div>
          <div id="popup-message" className="message">
            {message}
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

        {!isEyeRest && (
          <div className="countdown" aria-hidden="true">
            <div
              className="countdown-bar"
              style={{ animationDuration: `${AUTO_DISMISS_MS}ms` }}
              onAnimationEnd={() => act("dismissed")}
            />
          </div>
        )}
      </div>
    </div>
  );
}
