#!/usr/bin/env node
// test-calendar.mjs — a local ICS link to exercise every feature of Meeting Display.
//
//   node tools/test-calendar.mjs                 serve the calendar, print its link
//   node tools/test-calendar.mjs --use           also point the installed app at it (settings
//                                                backed up, app restarted) and put everything
//                                                back on Ctrl+C
//   node tools/test-calendar.mjs --scenario day  pick a scenario (default: quick)
//   node tools/test-calendar.mjs --list          describe the scenarios
//
// The link is http://127.0.0.1:<port>/test.ics : the app accepts plain HTTP for this machine only.
// Times are fixed when the server starts, so the meetings do not move while you test.
import { createServer } from "node:http";
import { spawn, execSync } from "node:child_process";
import { copyFileSync, existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const option = (name, fallback) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
};
const PORT = Number(option("--port", 8799));
const MIN = 60_000;

// Each event: minutes from server start, length in minutes, and optional Outlook status.
// status: BUSY (default), TENTATIVE (shown, no LED), FREE (hidden), or "cancelled", "allday".
const SCENARIOS = {
  quick: {
    about: "une réunion dans 6 min (approche, compte à rebours, START, LIVE, dépassement) en 12 min environ",
    events: [
      { in: 6, for: 3, subject: "Réunion de test" },
      { in: 14, for: 5, subject: "Réunion suivante" },
    ],
  },
  day: {
    about: "une journée chargée : enchaînements, réunion provisoire, créneaux à ne pas afficher, lendemain",
    events: [
      { in: 17, for: 20, subject: "Point d'équipe" },
      { in: 37, for: 15, subject: "Enchaînement sans pause" },
      { in: 70, for: 30, subject: "Atelier à confirmer", status: "TENTATIVE" },
      { in: 90, for: 30, subject: "Créneau libre (ne doit pas apparaître)", status: "FREE" },
      { in: 100, for: 30, subject: "Réunion annulée (ne doit pas apparaître)", status: "cancelled" },
      { in: 0, for: 0, subject: "Journée entière (ne doit pas apparaître)", status: "allday" },
      { in: 180, for: 45, subject: "Revue avec un titre vraiment très long pour vérifier le défilement sur la barre" },
      { in: 24 * 60 + 30, for: 30, subject: "Réunion de demain" },
      { in: 3 * 24 * 60, for: 60, subject: "Réunion dans trois jours" },
    ],
  },
  overrun: {
    about: "une réunion d'une minute qui commence dans 1 min, pour voir START puis le dépassement très vite",
    events: [{ in: 1, for: 1, subject: "Réunion éclair" }],
  },
  accents: {
    about: "des titres avec accents et symboles, pour vérifier la conversion en ASCII sur la barre",
    events: [
      { in: 8, for: 10, subject: "Été à Noël : café & crème brûlée — « ça va ? »" },
      { in: 30, for: 10, subject: "Réunion n°2 · point d’étape (1/2)" },
    ],
  },
  empty: { about: "aucune réunion : le panneau dit que l'agenda est libre, la barre rend la main à l'horloge", events: [] },
};

if (flag("--list")) {
  for (const [name, scenario] of Object.entries(SCENARIOS)) console.log(`${name.padEnd(8)} ${scenario.about}`);
  process.exit(0);
}
const scenarioName = option("--scenario", "quick");
const scenario = SCENARIOS[scenarioName];
if (!scenario) {
  console.error(`Scénario inconnu : ${scenarioName}. Disponibles : ${Object.keys(SCENARIOS).join(", ")}`);
  process.exit(1);
}

const t0 = Date.now();
const stamp = (ms) => new Date(ms).toISOString().replace(/[-:]/g, "").replace(/\.\d+Z$/, "Z");
const day = (ms) => {
  const d = new Date(ms);
  return `${d.getFullYear()}${String(d.getMonth() + 1).padStart(2, "0")}${String(d.getDate()).padStart(2, "0")}`;
};
const escape = (text) => text.replace(/\\/g, "\\\\").replace(/;/g, "\\;").replace(/,/g, "\\,");
const lines = ["BEGIN:VCALENDAR", "VERSION:2.0", "PRODID:-//meeting-display//test-calendar//FR"];
scenario.events.forEach((event, index) => {
  const start = t0 + event.in * MIN;
  lines.push("BEGIN:VEVENT", `UID:test-${scenarioName}-${index}@meeting-display.test`, `DTSTAMP:${stamp(t0)}`);
  if (event.status === "allday") lines.push(`DTSTART;VALUE=DATE:${day(t0)}`, `DTEND;VALUE=DATE:${day(t0 + 24 * 60 * MIN)}`);
  else lines.push(`DTSTART:${stamp(start)}`, `DTEND:${stamp(start + event.for * MIN)}`);
  lines.push(`SUMMARY:${escape(event.subject)}`);
  if (event.status === "cancelled") lines.push("STATUS:CANCELLED");
  const busy = ["TENTATIVE", "FREE"].includes(event.status) ? event.status : "BUSY";
  lines.push(`TRANSP:${busy === "FREE" ? "TRANSPARENT" : "OPAQUE"}`, `X-MICROSOFT-CDO-BUSYSTATUS:${busy}`, "END:VEVENT");
});
lines.push("END:VCALENDAR", "");
const ics = lines.join("\r\n");
const link = `http://127.0.0.1:${PORT}/test.ics`;

// --- pointing the installed app at the test calendar, and back
const settingsPath = join(process.env.APPDATA ?? "", "com.mokoguy.meeting-display", "settings.json");
const backupPath = `${settingsPath}.test-calendar-backup`;
const appExe = join(process.env.LOCALAPPDATA ?? "", "Meeting Display", "meeting-display.exe");
const restartApp = () => {
  try {
    execSync("taskkill /F /IM meeting-display.exe", { stdio: "ignore" });
  } catch {
    // not running
  }
  if (existsSync(appExe)) spawn(appExe, [], { detached: true, stdio: "ignore" }).unref();
  else console.log("Meeting Display n'est pas installé : lance l'app toi-même.");
};
let restored = false;
const restore = () => {
  if (restored || !existsSync(backupPath)) return;
  restored = true;
  copyFileSync(backupPath, settingsPath);
  rmSync(backupPath);
  restartApp();
  console.log("\nRéglages d'origine restaurés, app relancée.");
};

const server = createServer((request, response) => {
  if (!request.url?.startsWith("/test.ics")) {
    response.writeHead(404).end();
    return;
  }
  response.writeHead(200, { "Content-Type": "text/calendar; charset=utf-8", "Cache-Control": "no-store" });
  response.end(ics);
}).listen(PORT, "127.0.0.1", () => {
  console.log(`Scénario « ${scenarioName} » : ${scenario.about}`);
  for (const event of scenario.events) {
    if (event.status === "allday") continue;
    const at = new Date(t0 + event.in * MIN).toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" });
    console.log(`  ${at}  ${event.subject}${event.status ? `  [${event.status}]` : ""}`);
  }
  console.log(`\nLien ICS de test : ${link}`);
  if (flag("--use")) {
    if (!existsSync(settingsPath)) {
      console.error("Réglages de l'app introuvables : lance Meeting Display une première fois.");
      server.close();
      process.exit(1);
    }
    if (existsSync(backupPath)) copyFileSync(backupPath, settingsPath); // a previous run was killed: start from the real settings
    copyFileSync(settingsPath, backupPath);
    const settings = JSON.parse(readFileSync(settingsPath, "utf8"));
    writeFileSync(settingsPath, JSON.stringify({ ...settings, ics_url: link }));
    restartApp();
    console.log("L'app installée lit maintenant ce calendrier. Ctrl+C pour remettre tes réglages.");
  } else {
    console.log("Colle ce lien dans Réglages > Lien du calendrier, ou relance avec --use. Ctrl+C pour arrêter.");
  }
});

for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
  process.on(signal, () => {
    restore();
    server.close();
    process.exit(0);
  });
}
process.on("exit", restore);
