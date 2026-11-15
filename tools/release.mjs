#!/usr/bin/env node
// release.mjs — build the signed installer on this machine and, with --publish,
// attach it to a GitHub release (same result as the Release workflow, without CI).
//
//   node tools/release.mjs             build only: installer + signature + latest.json
//   node tools/release.mjs --publish   also create the GitHub release v<version>
//
// The update signing key is read from TAURI_SIGNING_PRIVATE_KEY, or from
// ~/.tauri/meeting-display.key. Publishing needs the GitHub CLI (gh), logged in.
import { execFileSync, execSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const app = join(root, "app");
const conf = JSON.parse(readFileSync(join(app, "src-tauri", "tauri.conf.json"), "utf8"));
const version = conf.version;
const tag = `v${version}`;

const keyFile = join(homedir(), ".tauri", "meeting-display.key");
const env = { ...process.env, TAURI_SIGNING_PRIVATE_KEY_PASSWORD: process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ?? "" };
if (!env.TAURI_SIGNING_PRIVATE_KEY) {
  if (!existsSync(keyFile)) {
    console.error(`Clé de signature introuvable : définis TAURI_SIGNING_PRIVATE_KEY ou place la clé dans ${keyFile}`);
    process.exit(1);
  }
  env.TAURI_SIGNING_PRIVATE_KEY = readFileSync(keyFile, "utf8").trim();
}

console.log(`Build de Meeting Display ${version}…`);
execSync("npm run tauri build", { cwd: app, env, stdio: "inherit" });

const nsis = join(app, "src-tauri", "target", "release", "bundle", "nsis");
const installer = join(nsis, `${conf.productName}_${version}_x64-setup.exe`);
const signature = `${installer}.sig`;
for (const file of [installer, signature]) {
  if (!existsSync(file)) {
    console.error(`Fichier attendu absent : ${file}`);
    process.exit(1);
  }
}

// GitHub replaces spaces in asset names with dots.
const assetName = `${conf.productName}_${version}_x64-setup.exe`.replace(/ /g, ".");
const repo = execFileSync("gh", ["repo", "view", "--json", "nameWithOwner", "--jq", ".nameWithOwner"], { cwd: root, encoding: "utf8" }).trim();
const latest = {
  version,
  notes: `Meeting Display ${version}`,
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": {
      signature: readFileSync(signature, "utf8").trim(),
      url: `https://github.com/${repo}/releases/download/${tag}/${assetName}`,
    },
  },
};
const latestFile = join(nsis, "latest.json");
writeFileSync(latestFile, JSON.stringify(latest, null, 2) + "\n");
console.log(`\nInstalleur : ${installer}\nSignature  : ${signature}\nManifeste  : ${latestFile}`);

if (process.argv.includes("--publish")) {
  execFileSync("gh", ["release", "create", tag, installer, signature, latestFile, "--title", `Meeting Display ${version}`,
    "--notes", "Installeur Windows. Les versions installées se mettent à jour seules."], { cwd: root, stdio: "inherit" });
  console.log(`Release ${tag} publiée sur ${repo}.`);
} else {
  console.log("\nAjoute --publish pour créer la release GitHub.");
}
