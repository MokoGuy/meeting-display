// Meeting Display, on-bar companion.
//
// The Windows app is the brain: it reads the calendar and writes a small agenda
// file into this app's data folder. This app is the display and the hands: it
// draws the meeting screens, lets the wheel browse today's meetings, and writes
// the user's actions (start, skip, end, resume) for the Windows app to apply.
//
// Files, in this app's data folder (the Windows app sees it as
// /ext/apps_data/js_apps/<app id>/):
//   agenda.json   written by the PC   what to show
//   actions.txt   appended by us      "<seq>|<action>|<meeting id>" per line
//   alive.txt     rewritten by us     heartbeat, so the PC knows we own the screen
import { device } from "@shared/device";
import manifest from "./appmeta/manifest.json";

const APP = manifest.id;
const API = "http://localhost/api";
const AGENDA = "/ext/data/agenda.json";
const ACTIONS = "/ext/data/actions.txt";
const ALIVE = "/ext/data/alive.txt";

const POLL_MS = 3000; // how often the agenda file is read
const TICK_MS = 1000; // how often the screen is reconsidered (drawn only when it changed)
const ALIVE_MS = 20000;
const KEEPALIVE_MS = 30000; // an unchanged screen is re-sent this often
const TIMEOUT_S = 60; // elements vanish by themselves if the app dies
const BROWSE_IDLE_MS = 10000; // the wheel view closes after this long without input
const FLASH_MS = 1500;

const WHITE = "#FFFFFFFF";
const DIM = "#FFFFFF90";
const FAINT = "#FFFFFF55";
const AMBER = "#FFB000FF";
const RED = "#FF5050FF";
const ORANGE = "#FFA500FF";
const GREEN = "#40E070FF";
const ACCENT = "#EA5212FF";
const GREY = "#FFFFFF70";

type ItemState = "up" | "tent" | "done" | "skip";
interface Item {
  id: string;
  t: string; // subject, ASCII
  hm: string; // "15:30"
  st: ItemState;
}
interface Cur {
  id: string;
  t: string;
  hm: string;
  lbl: string; // "15:30" today, "lun 12:30" another day
  s: number; // start, epoch seconds
  e: number; // end, epoch seconds
  phase: "upcoming" | "armed" | "live" | "overrun";
  badge: number;
  tent: boolean;
  today: boolean;
}
interface Agenda {
  seq: number;
  cur: Cur | null;
  day: Item[];
}

// Loose on purpose: these objects go straight to the display API.
type El = Record<string, unknown> & { id: string };

let agenda: Agenda = { seq: -1, cur: null, day: [] };
let browse = -1; // index in agenda.day, or -1 when the wheel view is closed
let lastInput = 0;
let flash = "";
let flashUntil = 0;
let lastSignature = "";
let lastDrawAt = 0;
let lastIds: string[] = [];
let drawing = false;

const nowS = () => Math.floor(Date.now() / 1000);

function report(err: unknown): void {
  console.error(`${APP}: ${err instanceof Error ? err.message : String(err)}`);
}

// ---------- element helpers
function rect(id: string, x: number, y: number, w: number, h: number, colors: string[]): El {
  return {
    id,
    type: "rectangle",
    display: "front",
    x,
    y,
    width: w,
    height: h,
    fill: colors.length > 1 ? "gradient_v" : "solid",
    fill_colors: colors,
    border_width: 0,
    timeout: TIMEOUT_S,
  };
}
function text(id: string, value: string, font: string, x: number, y: number, color: string, extra: Record<string, unknown> = {}): El {
  return { id, type: "text", display: "front", text: value, font, x, y, color, timeout: TIMEOUT_S, ...extra };
}
const SCROLL = { scroll_rate: 1200, scroll_start_delay: 1500, scroll_repeat_delay: 2500 };
function countdown(timestamp: number, direction: string, color: string, x: number): El {
  return {
    id: "cd",
    type: "countdown",
    display: "front",
    timestamp: String(timestamp),
    direction,
    show_hours: "when_non_zero",
    color,
    x,
    y: 19,
    align: "bottom_left",
    timeout: TIMEOUT_S,
  };
}
function back(label: string, subject: string, detail: string): El[] {
  const line = (id: string, value: string, font: string, y: number): El => ({
    id,
    type: "text",
    display: "back",
    text: value,
    font,
    x: 4,
    y,
    width: 136,
    color: WHITE,
    ...SCROLL,
    timeout: TIMEOUT_S,
  });
  return [line("back-title", label, "small", 8), line("back-subject", subject, "normal", 35), line("back-detail", detail, "small", 67)];
}

/** "2h05", "12 min": time left before a start, rounded up to the minute. */
function remaining(seconds: number): string {
  const minutes = Math.floor((Math.max(0, seconds) + 59) / 60);
  if (minutes >= 60) {
    const rest = minutes % 60;
    return `${Math.floor(minutes / 60)}h${rest < 10 ? "0" : ""}${rest}`;
  }
  return `${minutes} min`;
}

// ---------- the six meeting screens (same template as the Windows app draws)
function meetingScreen(cur: Cur): { elements: El[]; led?: string } {
  const toStart = cur.s - nowS();
  let kind: "idle" | "approach" | "imminent" | "armed" | "live" | "overrun";
  if (cur.phase === "armed") kind = "armed";
  else if (cur.phase === "live") kind = "live";
  else if (cur.phase === "overrun") kind = "overrun";
  else if (toStart > 15 * 60) kind = "idle";
  else if (toStart > 5 * 60) kind = "approach";
  else kind = "imminent";

  const bgs: Record<string, string[]> = {
    idle: ["#000000FF"],
    approach: ["#1E1200FF", "#000000FF"],
    imminent: ["#4A0808FF", "#140202FF"],
    armed: ["#5A0000FF", "#1E0000FF"],
    live: ["#002A12FF", "#000A04FF"],
    overrun: ["#2A1600FF", "#000000FF"],
  };
  const elements: El[] = [
    rect("bg", 0, 0, 72, 16, bgs[kind]),
    { id: "ico", type: "image", display: "front", path: "images/calendar.png", x: 1, y: 1, align: "top_left", timeout: TIMEOUT_S },
    text("badge", String(Math.min(99, cur.badge)), "superscript", 8, 6, "#323232FF", { align: "top_mid" }),
    text("title", cur.t, "small", 18, 0, kind === "idle" ? DIM : WHITE, { align: "top_left", width: 54, ...SCROLL }),
  ];
  const bl = (value: string, color: string) => elements.push(text("bl", value, "small", 18, 8, color, { align: "top_left" }));
  const br = (value: string, color: string) => elements.push(text("br", value, "small", 72, 8, color, { align: "top_right" }));
  let led: string | undefined;
  let label = "";
  let detail = "";
  if (kind === "idle") {
    bl(cur.lbl, DIM);
    if (cur.today) br(remaining(toStart), WHITE);
    label = "PROCHAIN";
    detail = `${cur.lbl} - dans ${remaining(toStart)}`;
  } else if (kind === "approach") {
    bl(cur.hm, DIM);
    br(remaining(toStart), AMBER);
    label = "BIENTOT";
    detail = `${cur.hm} - dans ${remaining(toStart)}`;
  } else if (kind === "imminent") {
    elements.push(countdown(cur.s, "time_left", WHITE, 18));
    br("IN", RED);
    if (toStart <= 60) led = "#E24B4AFF";
    label = "MAINTENANT";
    detail = `debut a ${cur.hm}`;
  } else if (kind === "armed") {
    bl(cur.hm, DIM);
    br("START", RED);
    led = "#FF0000FF";
    label = "C'EST L'HEURE";
    detail = `${cur.hm} - appuyer sur START`;
  } else if (kind === "live") {
    elements.push(countdown(cur.e, "time_left", WHITE, 18));
    br("LIVE", GREEN);
    label = "EN REUNION";
    detail = "START pour terminer";
  } else {
    bl("+", ORANGE);
    elements.push(countdown(cur.e, "time_since", ORANGE, 24));
    br("OVER", ORANGE);
    label = "DEPASSEMENT";
    detail = "START pour terminer";
  }
  elements.push(...back(label, cur.t, detail));
  return { elements, led: cur.tent ? undefined : led };
}

// ---------- the wheel view: the chosen meeting on top, the next one under it
function barColor(item: Item, chosen: boolean): string {
  if (item.st === "done") return GREEN;
  if (item.st === "skip") return GREY;
  return chosen ? ACCENT : GREY;
}
function browseScreen(day: Item[], index: number): { elements: El[] } {
  const chosen = day[index];
  const next = day[index + 1];
  const set = chosen.st === "done" || chosen.st === "skip";
  // Its own background id: the meeting screens use a gradient under "bg", and a gradient
  // replaced in place by a solid fill leaves a faint trace.
  const elements: El[] = [
    rect("list-bg", 0, 0, 72, 16, ["#000000FF"]),
    rect("b1", 0, 1, 2, 6, [barColor(chosen, true)]),
    text("t1", chosen.hm, "small", 4, -1, set ? DIM : WHITE),
    text("s1", chosen.t, "small", 31, -1, set ? DIM : WHITE, { width: 41, ...SCROLL }),
  ];
  if (next) {
    const nextSet = next.st === "done" || next.st === "skip";
    elements.push(
      rect("b2", 0, 10, 2, 6, [barColor(next, false)]),
      text("t2", next.hm, "small", 4, 8, nextSet ? FAINT : DIM),
      text("s2", next.t, "small", 31, 8, nextSet ? FAINT : DIM, { width: 41 }),
    );
  } else {
    elements.push(text("t2", "fin de journee", "small", 4, 8, FAINT));
  }
  const state = chosen.st === "done" ? "realisee" : chosen.st === "skip" ? "ignoree" : chosen.st === "tent" ? "provisoire" : "a venir";
  const hint = set ? "START pour reprendre" : "START demarrer - BACK ignorer";
  elements.push(...back(`${index + 1}/${day.length} - ${state}`, chosen.t, hint));
  return { elements };
}

function emptyScreen(): { elements: El[] } {
  return {
    elements: [
      rect("bg", 0, 0, 72, 16, ["#000000FF"]),
      text("title", "Agenda libre", "small", 36, 4, DIM, { align: "top_mid" }),
      ...back("MEETING DISPLAY", "Agenda libre", "aucune reunion a venir"),
    ],
  };
}

function flashScreen(message: string): { elements: El[] } {
  return {
    elements: [rect("bg", 0, 0, 72, 16, ["#000000FF"]), text("title", message, "bold", 36, 3, WHITE, { align: "top_mid" })],
  };
}

// ---------- drawing: only when something changed, one request at a time
async function draw(): Promise<void> {
  if (drawing) return;
  const screen =
    Date.now() < flashUntil
      ? flashScreen(flash)
      : browse >= 0 && agenda.day[browse]
        ? browseScreen(agenda.day, browse)
        : agenda.cur
          ? meetingScreen(agenda.cur)
          : emptyScreen();
  const led = "led" in screen ? (screen as { led?: string }).led : undefined;
  const signature = JSON.stringify(screen.elements) + (led ?? "");
  if (signature === lastSignature && Date.now() - lastDrawAt < KEEPALIVE_MS) return;
  drawing = true;
  try {
    const ids = screen.elements.map((element) => element.id);
    const gone = lastIds.filter((id) => ids.indexOf(id) < 0);
    if (gone.length > 0) await device.DisplayClear({ application_name: APP, element_ids: gone });
    const request: Record<string, unknown> = { application_name: APP, priority: 50, elements: screen.elements };
    // The LED blinks once when the screen appears, not at every keep-alive.
    if (led && signature !== lastSignature) request.led_notification_color = led;
    await device.DisplayDraw(request as unknown as Parameters<typeof device.DisplayDraw>[0]);
    lastIds = ids;
    lastSignature = signature;
    lastDrawAt = Date.now();
  } catch (err) {
    report(err);
  } finally {
    drawing = false;
  }
}

// ---------- files
async function readAgenda(): Promise<void> {
  try {
    const res = await fetch(`${API}/storage/read?path=${AGENDA}`);
    if (!res.ok) return;
    const next = JSON.parse(await res.text()) as Agenda;
    if (next.seq === agenda.seq) return;
    // Keep the wheel on the same meeting when the list changes under it.
    const chosenId = browse >= 0 && agenda.day[browse] ? agenda.day[browse].id : "";
    agenda = next;
    if (chosenId) {
      browse = -1;
      for (let i = 0; i < agenda.day.length; i++) if (agenda.day[i].id === chosenId) browse = i;
    }
    void draw();
  } catch (err) {
    report(err);
  }
}

async function post(path: string, body: string, append: boolean): Promise<void> {
  try {
    await fetch(`${API}/storage/write?path=${path}${append ? "&append=1" : ""}`, {
      method: "POST",
      headers: { "Content-Type": "application/octet-stream" },
      body,
    });
  } catch (err) {
    report(err);
  }
}

function act(action: string, id: string, message: string): void {
  void post(ACTIONS, `${Date.now()}|${action}|${id}\n`, true);
  flash = message;
  flashUntil = Date.now() + FLASH_MS;
  browse = -1;
  void draw();
}

// ---------- input
function chosen(): { item?: Item; isCur: boolean } {
  const cur = agenda.cur;
  if (browse >= 0 && agenda.day[browse]) {
    const item = agenda.day[browse];
    return { item, isCur: !!cur && cur.id === item.id };
  }
  if (!cur) return { isCur: false };
  return { item: { id: cur.id, t: cur.t, hm: cur.hm, st: cur.tent ? "tent" : "up" }, isCur: true };
}

function onStart(): void {
  const { item, isCur } = chosen();
  if (!item) return;
  const cur = agenda.cur;
  if (item.st === "done" || item.st === "skip") return act("resume", item.id, "Reprise");
  if (isCur && cur && (cur.phase === "live" || cur.phase === "overrun")) return act("end", item.id, "Terminee");
  act("start", item.id, "Demarree");
}

function onBack(): void {
  const { item, isCur } = chosen();
  if (!item || item.st === "done" || item.st === "skip") return;
  const cur = agenda.cur;
  if (isCur && cur && (cur.phase === "live" || cur.phase === "overrun")) return; // end it with START instead
  act("skip", item.id, "Ignoree");
}

function onWheel(delta: number): void {
  const count = agenda.day.length;
  if (count === 0) return;
  if (browse < 0) {
    // Open on the current meeting, or on the first one still to come.
    let start = 0;
    for (let i = 0; i < count; i++) {
      if (agenda.cur && agenda.day[i].id === agenda.cur.id) start = i;
    }
    browse = start;
  } else {
    browse = Math.min(count - 1, Math.max(0, browse + delta));
  }
  void draw();
}

function main(): void {
  void readAgenda();
  void post(ALIVE, String(Date.now()), false);
  void draw();

  setInterval(() => void readAgenda(), POLL_MS);
  setInterval(() => void post(ALIVE, String(Date.now()), false), ALIVE_MS);
  setInterval(() => {
    if (browse >= 0 && Date.now() - lastInput > BROWSE_IDLE_MS) browse = -1;
    void draw();
  }, TICK_MS);

  // Long BACK is the firmware's way out of any app; short BACK is ours.
  listen("input", (event) => {
    lastInput = Date.now();
    if (event.key === "encoder") return onWheel(event.delta);
    if (event.action !== "short") return;
    if (event.key === "start") onStart();
    else if (event.key === "back") onBack();
    else if (event.key === "ok") {
      browse = -1;
      void draw();
    }
  });
}

main();
