import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ConnectionDto,
  MeetingDto,
  MeetingStatusDto,
  endMeeting,
  getAutoStart,
  getConnection,
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
  Disclosure,
  IconButton,
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

  const refresh = () => {
    setNow(Date.now());
    void getMeetingStatus().then(setStatus);
    void getMeetings().then(setMeetings);
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

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (viewRef.current !== "main") {
        setView("main");
      } else {
        void getCurrentWindow().hide();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
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
  const rowState = (meeting: MeetingDto): RowState =>
    status.dismissed[meeting.id] ?? (meeting.show_as === "tentative" ? "tentative" : "upcoming");
  // Meetings still to come, plus today's set-aside ones, which stay at their time in the list.
  const others = meetings
    .filter((meeting) => meeting.id !== current?.id)
    .filter(
      (meeting) =>
        new Date(meeting.end).getTime() > now || (status.dismissed[meeting.id] && dayKey(meeting.start) === today),
    )
    .sort((a, b) => new Date(a.start).getTime() - new Date(b.start).getTime());
  // Day groups in order; the current meeting leads its own day.
  const groups: { key: string; first: string; rows: MeetingDto[] }[] = [];
  for (const meeting of others) {
    const key = dayKey(meeting.start);
    const group = groups.find((entry) => entry.key === key);
    if (group) group.rows.push(meeting);
    else groups.push({ key, first: meeting.start, rows: [meeting] });
  }
  const currentDay = current ? dayKey(current.start) : today;
  if (current && !groups.some((group) => group.key === currentDay)) {
    groups.unshift({ key: currentDay, first: current.start, rows: [] });
  }
  groups.sort((a, b) => new Date(a.first).getTime() - new Date(b.first).getTime());
  const todayCount =
    others.filter((meeting) => dayKey(meeting.start) === today && !status.dismissed[meeting.id]).length +
    (current && currentDay === today ? 1 : 0);
  const line = current ? nowLine(status, now) : null;
  // Rows keep their place in time: the ones that started before the current meeting come above it.
  const isBeforeCurrent = (meeting: MeetingDto, groupKey: string) =>
    !!current && groupKey === currentDay && new Date(meeting.start).getTime() < new Date(current.start).getTime();
  const row = (meeting: MeetingDto) => (
    <MeetingRow
      key={meeting.id}
      time={hhmm(meeting.start)}
      subject={meeting.subject}
      state={rowState(meeting)}
      onResume={() => void act(() => resumeMeeting(meeting.id))}
    />
  );

  return (
    <main className="app md-panel">
      {view === "main" ? (
        <div className="view">
          <header className="md-header">
            <span className="md-caps is-date">{longDate(now)}</span>
            <span className="md-header-count">
              {todayCount === 0 ? "Journée libre" : `${todayCount} réunion${todayCount > 1 ? "s" : ""}`}
            </span>
          </header>

          <div className="agenda">
            {groups.length === 0 && (
              <p className="empty">
                {icsUrl.trim()
                  ? "Aucune réunion dans les 7 prochains jours."
                  : "Ajoute le lien ICS de ton calendrier dans les réglages pour voir tes réunions ici et sur la barre."}
              </p>
            )}
            {groups.map((group) => (
              <section key={group.key}>
                {group.key !== today && <div className="md-caps md-group-title">{groupTitle(group.first, now)}</div>}
                {group.rows.filter((meeting) => isBeforeCurrent(meeting, group.key)).map(row)}
                {current && line && group.key === currentDay && (
                  <NowRow
                    subject={current.subject}
                    time={`${hhmm(current.start)} – ${hhmm(current.end)}${current.show_as === "tentative" ? " · provisoire" : ""}`}
                    state={line.state}
                    live={status.phase === "live"}
                  >
                    <Button primary onClick={() => void act(line.onPrimary)}>
                      {line.primary}
                    </Button>
                    {line.canSkip && <Button onClick={() => void act(skipMeeting)}>Ignorer</Button>}
                  </NowRow>
                )}
                {group.rows.filter((meeting) => !isBeforeCurrent(meeting, group.key)).map(row)}
              </section>
            ))}
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
