// Browser preview of the panel without the Tauri shell: `npm run dev`, then open
// http://localhost:1420/?phase=armed (none | upcoming | armed | live | overrun).
// Loaded only in development when the page is not hosted by Tauri.
import { mockIPC } from "@tauri-apps/api/mocks";

const minutes = (n: number) => new Date(Date.now() + n * 60000).toISOString();

function meeting(id: string, subject: string, startIn: number, length: number, showAs = "opaque") {
  return {
    id,
    subject,
    start: minutes(startIn),
    end: minutes(startIn + length),
    is_online: false,
    join_url: null,
    show_as: showAs,
    response: "none",
    is_cancelled: false,
    is_all_day: false,
  };
}

export function installMocks() {
  const phase = new URLSearchParams(window.location.search).get("phase") ?? "upcoming";
  const current = {
    upcoming: meeting("m1", "Point projet", 42, 30),
    armed: meeting("m1", "Point projet", -2, 30),
    live: meeting("m1", "Point projet", -12, 30),
    overrun: meeting("m1", "Point projet", -38, 30),
  }[phase];
  const earlier = [meeting("d1", "Standup équipe", -200, 15), meeting("d2", "Déjeuner d'équipe", -90, 60)];
  const later = [
    meeting("m2", "Standup équipe", 95, 15),
    meeting("m3", "Revue de sprint avec un titre vraiment très long", 180, 60, "tentative"),
    meeting("m4", "Point individuel", 300, 30),
    meeting("m5", "Standup équipe", 60 * 26, 15),
    meeting("m6", "Comité de pilotage", 60 * 28, 60),
  ];
  const settings = {
    base_url: "http://10.0.4.20",
    ics_url: phase === "none" ? "" : "https://example.test/calendar.ics",
    busy_session_policy: "interrupt",
    trigger_smart_home: false,
    quiet_hours_start: "",
    quiet_hours_end: "",
    discreet_mode: false,
    auto_start: false,
  };
  mockIPC((command) => {
    switch (command) {
      case "get_connection":
        return { connected: true, name: "Ma barre", battery: 92, power_state: "charging" };
      case "get_settings":
        return settings;
      case "get_meeting_status":
        return { phase: current ? phase : "none", meeting: current ?? null, dismissed: phase === "upcoming" ? { d1: "done", d2: "skipped" } : {} };
      case "get_meetings":
        return current ? [...earlier, current, ...later] : [];
      case "get_versions":
        return { app: "0.5.4", update: "à jour", bar_app: "0.5.4", bar_reachable: true };
      case "get_history": {
        if (!current) return [];
        const entry = (id: string, subject: string, startIn: number, planned: number, actual: number | null) => ({
          id,
          subject,
          start: minutes(startIn),
          planned_secs: planned * 60,
          state: actual === null ? "skipped" : "done",
          started_at: actual === null ? null : minutes(startIn + 1),
          ended_at: actual === null ? null : minutes(startIn + 1 + actual),
        });
        return [
          entry("h1", "Revue de sprint", -3 * 1440 - 240, 60, 72),
          entry("h2", "Standup équipe", -3 * 1440 - 480, 15, 16),
          entry("h3", "Point individuel", -3 * 1440 - 90, 30, null),
          entry("d1", "Standup équipe", -200, 15, 14),
          entry("d2", "Déjeuner d'équipe", -90, 60, null),
        ];
      }
      case "get_autostart":
        return false;
      case "test_connection":
        return "Ma barre";
      default:
        // events, window calls and write commands: accepted, no effect
        return null;
    }
  });
}
