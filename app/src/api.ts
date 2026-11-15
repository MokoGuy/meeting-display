import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface ConnectionDto {
  connected: boolean;
  name: string | null;
  battery: number | null;
  power_state: string | null;
}

export const sendStart = () => invoke<void>("send_start");
export const sendKey = (key: string) => invoke<void>("send_key", { key });

export const getConnection = () => invoke<ConnectionDto>("get_connection");

export const onConnectionChanged = (
  callback: (connection: ConnectionDto) => void,
): Promise<UnlistenFn> =>
  listen<ConnectionDto>("connection-changed", (event) => callback(event.payload));

export interface SettingsDto {
  base_url: string;
  ics_url: string;
  busy_session_policy: string;
  trigger_smart_home: boolean;
  quiet_hours_start: string;
  quiet_hours_end: string;
  discreet_mode: boolean;
  auto_start: boolean;
}

export const getSettings = () => invoke<SettingsDto>("get_settings");
export const saveSettings = (base_url: string) => invoke<void>("save_settings", { baseUrl: base_url });
export const testConnection = (base_url: string) => invoke<string>("test_connection", { baseUrl: base_url });
export const openBarUi = () => invoke<void>("open_bar_ui");
export const getAutoStart = () => invoke<boolean>("get_autostart");
export const setAutoStart = (enabled: boolean) => invoke<void>("set_autostart", { enabled });

export interface MeetingDto {
  id: string;
  subject: string;
  start: string;
  end: string;
  is_online: boolean;
  join_url: string | null;
  show_as: string;
  response: string;
  is_cancelled: boolean;
  is_all_day: boolean;
}
export const getMeetings = () => invoke<MeetingDto[]>("get_meetings");
export const onMeetingsChanged = (callback: (meetings: MeetingDto[]) => void): Promise<UnlistenFn> =>
  listen<MeetingDto[]>("meetings-changed", (event) => callback(event.payload));
export const openJoinUrl = (url: string) => invoke<void>("open_join_url", { url });
export const saveCalendarSettings = (icsUrl: string) =>
  invoke<void>("save_calendar_settings", { icsUrl });

/** Policy and smart-home are kept for compatibility; the app no longer uses them. */
export const saveMeetingSettings = (
  policy: string,
  triggerSmartHome: boolean,
  quietHoursStart: string,
  quietHoursEnd: string,
  discreetMode: boolean,
  autoStart: boolean,
) =>
  invoke<void>("save_meeting_settings", {
    policy,
    triggerSmartHome,
    quietHoursStart,
    quietHoursEnd,
    discreetMode,
    autoStart,
  });

export type MeetingPhase = "none" | "upcoming" | "armed" | "live" | "overrun";
export interface MeetingStatusDto {
  phase: MeetingPhase;
  meeting: MeetingDto | null;
  /** Meetings set aside by hand: id to how. */
  dismissed: Record<string, "done" | "skipped">;
}
export const getMeetingStatus = () => invoke<MeetingStatusDto>("get_meeting_status");

/** Goes live now: the armed meeting, or the next one ahead of time. */
export const startMeetingNow = () => invoke<void>("start_meeting_now");
/** Ignores the armed meeting, or else the next upcoming one. */
export const skipMeeting = () => invoke<void>("skip_meeting");
/** Ends the meeting in progress (or its overrun). */
export const endMeeting = () => invoke<void>("end_meeting");
/** Puts one meeting skipped or ended by hand back in play. */
export const resumeMeeting = (id: string) => invoke<void>("resume_meeting", { id });

/** One meeting held or skipped, as kept in the local 30-day history. */
export interface HistoryEntryDto {
  id: string;
  subject: string;
  /** Scheduled start, or the moment it was started when started ahead of time. */
  start: string;
  planned_secs: number;
  state: "done" | "skipped";
  started_at: string | null;
  ended_at: string | null;
}
export const getHistory = () => invoke<HistoryEntryDto[]>("get_history");
