// Runs after `npm install`. On Windows, @busy-app/cli 0.6.0 writes font-map
// import paths with backslashes, which the bundler then reads as escape
// sequences ("Rolldown failed to resolve import ... #font-maps"). This rewrites
// that one line to use forward slashes. Harmless elsewhere, and a no-op once
// the tool is fixed upstream.
const fs = require("fs");
const path = require("path");

const file = path.join(__dirname, "..", "node_modules", "@busy-app", "cli", "dist", "lib", "fontMapsPlugin.js");
const broken = "import f${i} from '${resolve(maps, `${f}.json`)}';";
const fixed = "import f${i} from '${resolve(maps, `${f}.json`).split(\"\\\\\").join(\"/\")}';";

if (!fs.existsSync(file)) process.exit(0);
const source = fs.readFileSync(file, "utf8");
if (source.includes(broken)) {
  fs.writeFileSync(file, source.replace(broken, fixed));
  console.log("bar-app: patched @busy-app/cli font-map paths for Windows");
}
