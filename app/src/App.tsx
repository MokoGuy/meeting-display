import { useEffect, useRef, useState, type ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ConnectionDto,
  HistoryEntryDto,
  MeetingDto,
  MeetingStatusDto,
  endMeeting,
  getAutoStart,
  getConnection,
  getHistory,
  getMeetingStatus,
  getMeetings,
  getSettings,
  onConnectionChanged,
  onMeetingsChanged,
  resumeMeeting,
  saveCalendarSettings,
  saveMeetingSettings,
  saveSettings,
  setAutoStart,
  skipMeeting,
  startMeetingNow,
  testConnection,
} from "./api";
import {
  Button,
  DayNav,
  Disclosure,
  IconButton,
  IconPill,
  MeetingRow,
  NowRow,
  StatusDot,
  Switch,
  TextField,
  ThemeButton,
  TimeField,
  type RowState,
  type Theme,
} from "./ui/components";
import "./App.css";

const initialConnection: ConnectionDto = {
  connected: false,
  name: null,
  battery: null,
  power_state: null,
};

type View = "main" | "settings";

const hhmm = (iso: string) =>
  new Date(iso).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

const dayKey = (iso: string | number) => new Date(iso).toDateString();

const longDate = (iso: string | number) =>
  new Date(iso).toLocaleDateString("fr-FR", { weekday: "long", day: "numeric", month: "long" });

function groupTitle(iso: string, now: number) {
  const tomorrow = new Date(now);
  tomorrow.setDate(tomorrow.getDate() + 1);
  return dayKey(iso) === dayKey(tomorrow.getTime()) ? "Demain" : longDate(iso);
}

/** "12 min", "2 h 05", "3 j". */
function duration(ms: number) {
  const minutes = Math.max(0, Math.ceil(ms / 60000));
  if (minutes < 60) return `${minutes} min`;
  if (minutes < 24 * 60) return `${Math.floor(minutes / 60)} h ${String(minutes % 60).padStart(2, "0")}`;
  return `${Math.floor(minutes / 1440)} j`;
}

/** A length in seconds as "16 min" or "1 h 12". */
function span(seconds: number) {
  const minutes = Math.max(seconds > 0 ? 1 : 0, Math.round(seconds / 60));
  if (minutes < 60) return `${minutes} min`;
  const rest = minutes % 60;
  return rest === 0 ? `${Math.floor(minutes / 60)} h` : `${Math.floor(minutes / 60)} h ${String(rest).padStart(2, "0")}`;
}

/** How long a recorded meeting really lasted, in seconds; null when it was skipped. */
function actualSeconds(entry: HistoryEntryDto) {
  if (!entry.started_at || !entry.ended_at) return null;
  return Math.max(0, (new Date(entry.ended_at).getTime() - new Date(entry.started_at).getTime()) / 1000);
}

/** How far the panel lets you travel from today, in days. */
const DAYS_BACK = 30;
const DAYS_AHEAD = 7;

/** The state line and the actions of the expanded row, for each phase. */
function nowLine(status: MeetingStatusDto, now: number) {
  const meeting = status.meeting!;
  const start = new Date(meeting.start).getTime();
  const end = new Date(meeting.end).getTime();
  switch (status.phase) {
    case "armed":
      return { state: "C'est l'heure", primary: "Démarrer", onPrimary: startMeetingNow, canSkip: true };
    case "live":
      return { state: `En réunion · encore ${duration(end - now)}`, primary: "Terminer", onPrimary: endMeeting, canSkip: false };
    case "overrun":
      return { state: `Dépassement · + ${duration(now - end)}`, primary: "Terminer", onPrimary: endMeeting, canSkip: false };
    default:
      return { state: `dans ${duration(start - now)}`, primary: "Démarrer", onPrimary: startMeetingNow, canSkip: true };
  }
}

function App() {
  const [view, setView] = useState<View>("main");
  const [connection, setConnection] = useState(initialConnection);
  const [status, setStatus] = useState<MeetingStatusDto>({ phase: "none", meeting: null, dismissed: {} });
  const [meetings, setMeetings] = useState<MeetingDto[]>([]);
  const [history, setHistory] = useState<HistoryEntryDto[]>([]);
  // 0 is today, -1 yesterday, +1 tomorrow.
  const [dayOffset, setDayOffset] = useState(0);
  const [now, setNow] = useState(Date.now());
  const [message, setMessage] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [icsUrl, setIcsUrl] = useState("");
  // Stored for compatibility; no longer shown nor used by the app.
  const [policy, setPolicy] = useState("interrupt");
  const [smartHome, setSmartHome] = useState(false);
  const [quietStart, setQuietStart] = useState("");
  const [quietEnd, setQuietEnd] = useState("");
  const [discreet, setDiscreet] = useState(false);
  const [autoLive, setAutoLive] = useState(false);
  const [autoStart, setAutoStartState] = useState(false);
  const [theme, setTheme] = useState<Theme>(() => {
    try {
      const saved = window.localStorage.getItem("theme");
      return saved === "light" || saved === "dark" ? saved : "auto";
    } catch {
      return "auto";
    }
  });
  const viewRef = useRef(view);
  viewRef.current = view;
  const dayOffsetRef = useRef(dayOffset);
  dayOffsetRef.current = dayOffset;

  const refresh = () => {
    setNow(Date.now());
    void getMeetingStatus().then(setStatus);
    void getMeetings().then(setMeetings);
    void getHistory().then(setHistory);
  };

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let unlistenMeetings: (() => void) | undefined;
    void getConnection().then(setConnection);
    void onConnectionChanged(setConnection).then((cleanup) => {
      unlisten = cleanup;
    });
    void getSettings().then((settings) => {
      setBaseUrl(settings.base_url);
      setIcsUrl(settings.ics_url);
      setPolicy(settings.busy_session_policy);
      setSmartHome(settings.trigger_smart_home);
      setQuietStart(settings.quiet_hours_start);
      setQuietEnd(settings.quiet_hours_end);
      setDiscreet(settings.discreet_mode);
      setAutoLive(settings.auto_start);
    });
    void onMeetingsChanged(() => refresh()).then((cleanup) => {
      unlistenMeetings = cleanup;
    });
    void getAutoStart().then(setAutoStartState);
    refresh();
    const timer = window.setInterval(refresh, 5000);

    const step = (delta: number) =>
      setDayOffset((offset) => Math.min(DAYS_AHEAD, Math.max(-DAYS_BACK, offset + delta)));
    const onKeyDown = (event: KeyboardEvent) => {
      const typing = event.target instanceof HTMLInputElement;
      if (viewRef.current === "main" && !typing && event.key === "ArrowLeft") return step(-1);
      if (viewRef.current === "main" && !typing && event.key === "ArrowRight") return step(1);
      if (event.key !== "Escape") return;
      if (viewRef.current !== "main") {
        setView("main");
      } else if (dayOffsetRef.current !== 0) {
        setDayOffset(0);
      } else {
        void getCurrentWindow().hide();
      }
    };
    // The panel always reopens on today.
    const onFocus = () => setDayOffset(0);
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("focus", onFocus);
    return () => {
      window.removeEventListener("focus", onFocus);
      unlisten?.();
      unlistenMeetings?.();
      window.clearInterval(timer);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, []);

  // The theme is an attribute on the root element; "auto" removes it and Windows decides.
  useEffect(() => {
    if (theme === "auto") delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = theme;
    try {
      window.localStorage.setItem("theme", theme);
    } catch {
      // the choice simply is not remembered
    }
  }, [theme]);

  async function act(action: () => Promise<void>) {
    setMessage("");
    try {
      await action();
    } catch (error) {
      setMessage(String(error));
    } finally {
      refresh();
    }
  }

  async function save() {
    try {
      await saveSettings(baseUrl);
      await saveCalendarSettings(icsUrl);
      await saveMeetingSettings(policy, smartHome, quietStart, quietEnd, discreet, autoLive);
      setMessage("Réglages enregistrés");
      refresh();
    } catch (error) {
      setMessage(`Échec : ${String(error)}`);
    }
  }

  async function test() {
    setMessage("Test en cours…");
    try {
      setMessage(`Barre trouvée : ${await testConnection(baseUrl)}`);
    } catch (error) {
      setMessage(`Échec : ${String(error)}`);
    }
  }

  async function toggleAutoStart(enabled: boolean) {
    try {
      await setAutoStart(enabled);
      setAutoStartState(enabled);
    } catch (error) {
      setMessage(`Échec : ${String(error)}`);
    }
  }

  const current = status.meeting;
  const today = dayKey(now);
  const shown = new Date(now);
  shown.setDate(shown.getDate() + dayOffset);
  const shownDay = dayKey(shown.getTime());
  const onToday = dayOffset === 0;

  // One list model for what was done (history) and what is coming (calendar).
  type Item = { id: string; subject: string; start: number; state: RowState; detail?: ReactNode; resumable: boolean };
  const recorded = new Set(history.map((entry) => entry.id));
  const past: Item[] = history
    .filter((entry) => entry.id !== current?.id)
    .map((entry) => {
      const actual = actualSeconds(entry);
      return {
        id: entry.id,
        subject: entry.subject,
        start: new Date(entry.start).getTime(),
        state: entry.state,
        detail:
          actual === null ? undefined : (
            <>
              prévu {span(entry.planned_secs)} · <strong>réel {span(actual)}</strong>
            </>
          ),
        // Today's meetings and later ones can be put back in play: a meeting skipped by mistake
        // days ahead must not be lost. Earlier days are history and stay as they are.
        resumable: new Date(entry.start).getTime() >= new Date(today).getTime(),
      };
    });
  const coming: Item[] = meetings
    .filter((meeting) => meeting.id !== current?.id && !recorded.has(meeting.id))
    .filter((meeting) => new Date(meeting.end).getTime() > now)
    .map((meeting) => ({
      id: meeting.id,
      subject: meeting.subject,
      start: new Date(meeting.start).getTime(),
      state: meeting.show_as === "tentative" ? "tentative" : "upcoming",
      resumable: false,
    }));
  // Today shows today's history and everything to come; any other day shows that day only.
  const items = [...past, ...coming]
    .filter((item) => (onToday ? item.start >= new Date(today).getTime() : dayKey(item.start) === shownDay))
    .sort((a, b) => a.start - b.start);
  const groups: { key: string; first: number; rows: Item[] }[] = [];
  for (const item of items) {
    const key = dayKey(item.start);
    const group = groups.find((entry) => entry.key === key);
    if (group) group.rows.push(item);
    else groups.push({ key, first: item.start, rows: [item] });
  }
  const currentDay = current ? dayKey(current.start) : today;
  const currentShown = !!current && (onToday || currentDay === shownDay);
  if (current && currentShown && !groups.some((group) => group.key === currentDay)) {
    groups.push({ key: currentDay, first: new Date(current.start).getTime(), rows: [] });
  }
  groups.sort((a, b) => a.first - b.first);
  const todayCount = coming.filter((item) => dayKey(item.start) === today).length + (current && currentDay === today ? 1 : 0);
  const line = current ? nowLine(status, now) : null;
  // Rows keep their place in time: the ones that started before the current meeting come above it.
  const isBeforeCurrent = (item: Item, groupKey: string) =>
    !!current && groupKey === currentDay && item.start < new Date(current.start).getTime();
  const row = (item: Item) => (
    <MeetingRow
      key={item.id}
      time={hhmm(new Date(item.start).toISOString())}
      subject={item.subject}
      state={item.state}
      detail={item.detail}
      onResume={item.resumable ? () => void act(() => resumeMeeting(item.id)) : undefined}
    />
  );
  // Total of a past day: meetings held, planned against actual.
  const held = history.filter((entry) => dayKey(entry.start) === shownDay && entry.state === "done");
  const summary =
    !onToday && held.length > 0
      ? `${held.length} réunion${held.length > 1 ? "s" : ""} réalisée${held.length > 1 ? "s" : ""} · prévu ${span(
          held.reduce((sum, entry) => sum + entry.planned_secs, 0),
        )} · réel ${span(held.reduce((sum, entry) => sum + (actualSeconds(entry) ?? 0), 0))}`
      : "";
  const emptyText = !icsUrl.trim()
    ? "Ajoute le lien ICS de ton calendrier dans les réglages pour voir tes réunions ici et sur la barre."
    : onToday
      ? "Aucune réunion dans les 7 prochains jours."
      : dayOffset < 0
        ? "Aucune réunion enregistrée ce jour-là."
        : "Aucune réunion ce jour-là.";

  return (
    <main className="app md-panel">
      {view === "main" ? (
        <div className="view">
          <DayNav
            label={longDate(shown.getTime())}
            today={onToday}
            onPrev={dayOffset > -DAYS_BACK ? () => setDayOffset(dayOffset - 1) : undefined}
            onNext={dayOffset < DAYS_AHEAD ? () => setDayOffset(dayOffset + 1) : undefined}
          >
            {onToday ? (
              todayCount === 0 ? (
                "Journée libre"
              ) : (
                `${todayCount} réunion${todayCount > 1 ? "s" : ""}`
              )
            ) : (
              <Button small onClick={() => setDayOffset(0)}>
                Aujourd'hui
              </Button>
            )}
          </DayNav>

          <div className="agenda">
            {groups.length === 0 && <p className="empty">{emptyText}</p>}
            {groups.map((group) => (
              <section key={group.key}>
                {onToday && group.key !== today && (
                  <div className="md-caps md-group-title">{groupTitle(new Date(group.first).toISOString(), now)}</div>
                )}
                {group.rows.filter((item) => isBeforeCurrent(item, group.key)).map(row)}
                {current && line && currentShown && group.key === currentDay && (
                  <NowRow
                    subject={current.subject}
                    time={`${hhmm(current.start)} – ${hhmm(current.end)}${current.show_as === "tentative" ? " · provisoire" : ""}`}
                    state={line.state}
                    live={status.phase === "live"}
                  >
                    <Button primary onClick={() => void act(line.onPrimary)}>
                      {line.primary}
                    </Button>
                    {line.canSkip && <IconPill icon="skip" label="Ignorer" onClick={() => void act(skipMeeting)} />}
                  </NowRow>
                )}
                {group.rows.filter((item) => !isBeforeCurrent(item, group.key)).map(row)}
              </section>
            ))}
            {summary && <div className="md-day-summary">{summary}</div>}
          </div>

          <footer className="md-footer">
            {message ? (
              <StatusDot>{message}</StatusDot>
            ) : (
              <StatusDot online={connection.connected}>
                {connection.connected ? "Barre connectée" : "Barre hors ligne"}
              </StatusDot>
            )}
            <ThemeButton theme={theme} onChange={setTheme} />
            <IconButton
              icon="settings"
              label="Réglages"
              onClick={() => {
                setMessage("");
                setView("settings");
              }}
            />
          </footer>
        </div>
      ) : (
        <div className="view">
          <div className="settings-header">
            <IconButton icon="back" label="Retour" onClick={() => setView("main")} />
            <h1>Réglages</h1>
          </div>
          <div className="settings">
            <TextField label="Lien du calendrier (ICS)" value={icsUrl} onChange={setIcsUrl} secret />
            <Switch checked={autoLive} onChange={setAutoLive} label="Passer en réunion automatiquement à l'heure" />
            <Switch checked={discreet} onChange={setDiscreet} label="Mode discret" hint="Rien sur la barre, aucun son" />
            <div className="time-row">
              <TimeField label="Silence de" value={quietStart} onChange={setQuietStart} />
              <TimeField label="à" value={quietEnd} onChange={setQuietEnd} />
            </div>
            <Switch checked={autoStart} onChange={(next) => void toggleAutoStart(next)} label="Lancer au démarrage de Windows" />
            <Disclosure label="Avancé">
              <TextField label="Adresse de la barre" value={baseUrl} onChange={setBaseUrl} />
              <div>
                <Button onClick={() => void test()}>Tester la connexion</Button>
              </div>
            </Disclosure>
          </div>
          <footer className="md-footer">
            <StatusDot>{message}</StatusDot>
            <Button primary onClick={() => void save()}>
              Enregistrer
            </Button>
          </footer>
        </div>
      )}
    </main>
  );
}

export default App;
