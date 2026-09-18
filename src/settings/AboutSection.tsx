import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UpdateInfo, UpdateStatus } from "../types";

function describe(status: UpdateStatus) {
  switch (status.state) {
    case "idle":
      return "Updates are checked automatically.";
    case "checking":
      return "Checking for updates…";
    case "upToDate":
      return "You're on the latest version.";
    case "available":
      return `Version ${status.version} is available.`;
    case "installing":
      return "Installing the update. Macy Health will restart.";
    case "failed":
      return status.message;
  }
}

export function AboutSection() {
  const [info, setInfo] = useState<UpdateInfo | null>(null);

  useEffect(() => {
    invoke<UpdateInfo>("get_update_info").then(setInfo);
    const unlisten = listen<UpdateStatus>("update:changed", (e) =>
      setInfo((current) => current && { ...current, status: e.payload }),
    );
    return () => {
      unlisten.then((off) => off());
    };
  }, []);

  if (!info) return null;
  const { status } = info;
  const busy = status.state === "checking" || status.state === "installing";

  return (
    <ul className="group">
      <li className="setting">
        <div className="setting-text">
          <span className="setting-title">Macy Health {info.currentVersion}</span>
          <span className="setting-help" role="status">
            {describe(status)}
          </span>
        </div>
        <div className="setting-control">
          {status.state === "available" ? (
            <button type="button" className="button primary" onClick={() => invoke("install_update")}>
              Update and restart
            </button>
          ) : (
            <button type="button" className="button" disabled={busy} onClick={() => invoke("check_for_updates")}>
              {status.state === "failed" ? "Try again" : "Check for updates"}
            </button>
          )}
        </div>
      </li>
    </ul>
  );
}
