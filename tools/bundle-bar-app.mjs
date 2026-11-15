#!/usr/bin/env node
// bundle-bar-app.mjs — puts the built on-bar app inside the Windows app.
//
// The Windows app carries the on-bar app's package and installs it on the bar when the bar
// has none or an older one. This copies bar-app/dist/meeting-display.tgz (made by
// `npm run build` in bar-app, from PowerShell) to app/src-tauri/assets/bar-app.tgz, after
// checking that the package really holds the version the Windows app is about to claim.
//
//   node tools/bundle-bar-app.mjs           copy, then check
//   node tools/bundle-bar-app.mjs --check   check only what is already bundled
import { copyFileSync, existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const built = join(root, "bar-app", "dist", "meeting-display.tgz");
const bundled = join(root, "app", "src-tauri", "assets", "bar-app.tgz");
const appVersion = JSON.parse(readFileSync(join(root, "app", "src-tauri", "tauri.conf.json"), "utf8")).version;

/** The version in the manifest packed inside a .tgz, or null. */
export function packagedVersion(file) {
  const tar = gunzipSync(readFileSync(file)).toString("latin1");
  const at = tar.indexOf('"id": "meeting-display"');
  if (at < 0) return null;
  const found = /"version":\s*"([^"]+)"/.exec(tar.slice(at, at + 400));
  return found ? found[1] : null;
}

/** Explains what is wrong with the bundled package, or returns "" when it matches the app. */
export function bundleProblem() {
  if (!existsSync(bundled)) return `absent : ${bundled}`;
  const version = packagedVersion(bundled);
  if (version !== appVersion) return `le paquet embarqué est en ${version}, l'app Windows en ${appVersion}`;
  return "";
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  if (!process.argv.includes("--check")) {
    if (!existsSync(built)) {
      console.error(`Paquet introuvable : ${built}\nConstruis-le d'abord : dans bar-app, depuis PowerShell, npm run build`);
      process.exit(1);
    }
    copyFileSync(built, bundled);
  }
  const problem = bundleProblem();
  if (problem) {
    console.error(`App de la barre non embarquée correctement : ${problem}.\nDans bar-app (PowerShell) : npm run build, puis node tools/bundle-bar-app.mjs`);
    process.exit(1);
  }
  console.log(`App de la barre ${appVersion} embarquée dans l'app Windows.`);
}
