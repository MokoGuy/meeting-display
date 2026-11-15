// Generates the transition animations in src/animations/. Run it with the bar plugged in:
//
//   node scripts/make-anims.mjs
//
// The bar plays an animation file at 60 frames per second by itself, where an app that
// redraws every frame manages about 12. So the transitions are overlays:
//
//   reveal_up / reveal_down   wheel: the next meeting arrives under a black curtain that
//                         lifts in the direction of travel
//   in_<word>             action: an orange arrow crosses left to right; behind it the
//                         word is already there, ahead of it the old screen shows through
//   out_<word>            the same arrow uncovers the next screen from under the word
//
// The words are fixed, so their pixels are baked into the files. They are captured from
// the bar itself, which is why it must be plugged in: that is the only way to get its font.
//
// File format: lib/anim_file/anim_file_format.h in busy-app/busybar-firmware ("bicycle1").
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const BAR = process.env.BUSYBAR_URL || "http://10.0.4.20";
const OUT = join(dirname(fileURLToPath(import.meta.url)), "..", "src", "animations");
const W = 72, H = 16, FPS = 60;
const ACCENT = [234, 82, 18]; // the app's orange, same as the panel's primary button
const WORDS = ["Demarree", "Terminee", "Ignoree", "Reprise"]; // the flash words of src/main.ts

const SAME = null; // a frame identical to the previous one: costs five bytes

function encode(frames) {
  const name = Buffer.from("default\0", "latin1");
  const sections = Buffer.alloc(12 + name.length);
  const chunk = Buffer.concat(
    frames.map((px) => {
      const head = Buffer.alloc(5);
      // joint encoding: mask in the top four bits (0 = nothing changed, 1 = every pixel present), raw pixels
      head.writeUInt8(px === SAME ? 0 : 1 << 4, 0);
      head.writeUInt16LE(0, 1);
      head.writeUInt16LE(px === SAME ? 0 : px.length, 3);
      return px === SAME ? head : Buffer.concat([head, px]);
    }),
  );
  const header = Buffer.alloc(34);
  header.write("bicycle1", 0, "latin1");
  header.writeUInt8(0, 8);
  header.writeUInt8(W, 9);
  header.writeUInt8(H, 10);
  header.writeUInt8(2, 11); // blue, green, red, alpha
  header.writeUInt16LE(0, 12);
  header.writeUInt16LE(W * H * 4, 14);
  header.writeUInt8(FPS, 16);
  header.writeUInt32LE(sections.length, 18);
  header.writeUInt32LE(chunk.length, 22);
  header.writeUInt32LE(1, 26);
  header.writeUInt32LE(frames.length, 30);
  sections.writeUInt32LE(0, 0);
  sections.writeUInt32LE(frames.length - 1, 4);
  sections.writeUInt32LE(header.length + sections.length, 8);
  name.copy(sections, 12);
  return Buffer.concat([header, sections, chunk]);
}

/** One frame from fn(x, y) -> [red, green, blue, alpha 0..1]. */
function frame(fn) {
  const px = Buffer.alloc(W * H * 4);
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      const [r, g, b, a] = fn(x, y);
      const at = (y * W + x) * 4;
      px[at] = b;
      px[at + 1] = g;
      px[at + 2] = r;
      px[at + 3] = Math.round(255 * Math.max(0, Math.min(1, a)));
    }
  }
  return px;
}
const ease = (t) => t * t * (3 - 2 * t);
const CLEAR = () => [0, 0, 0, 0];

// ---------- wheel: a black curtain, 0.05 s to cover, 0.22 s covered, 0.08 s to uncover.
// The covered time is what the app needs to put the next screen in place (two requests of
// about 90 ms each); everything else is kept as short as still reads as a movement.
const COVER = 3, COVERED = 13, UNCOVER = 5, FEATHER = 5;
/**
 * The fast variant the app uses: the curtain is already down on the first frame and only lifts.
 * It travels in the same request as the next screen, so one request is all a notch costs.
 */
function reveal(up) {
  const fromEntry = (y) => (up ? H - 1 - y : y);
  const frames = [frame(() => [0, 0, 0, 1]), SAME];
  for (let f = 0; f < LIFT; f++) {
    const edge = ease((f + 1) / LIFT) * (H + FEATHER);
    frames.push(frame((x, y) => [0, 0, 0, 1 - (edge - fromEntry(y)) / FEATHER]));
  }
  frames.push(frame(CLEAR));
  return frames;
}
const LIFT = 7;

/** The two-step variant, kept for reference: covers, waits for the app, uncovers. up = true: the curtain travels upward. */
// eslint-disable-next-line no-unused-vars
function wipe(up) {
  const fromEntry = (y) => (up ? H - 1 - y : y);
  const frames = [];
  for (let f = 0; f < COVER; f++) {
    const edge = ease((f + 1) / COVER) * (H + FEATHER);
    frames.push(frame((x, y) => [0, 0, 0, (edge - fromEntry(y)) / FEATHER]));
  }
  for (let f = 0; f < COVERED; f++) frames.push(SAME);
  for (let f = 0; f < UNCOVER; f++) {
    const edge = ease((f + 1) / UNCOVER) * (H + FEATHER);
    frames.push(frame((x, y) => [0, 0, 0, 1 - (edge - fromEntry(y)) / FEATHER]));
  }
  frames.push(frame(CLEAR));
  return frames;
}

// ---------- actions: an arrow head, 0.27 s to cross
const CROSS = 16, BAND = 12, TIP = 10;
const lag = (y) => Math.abs(y - 7.5) * 1.2 + 0.4; // rows away from the middle are late: the edge is a ">"
const pixel = (shot, x, y) => [shot[(y * W + x) * 3 + 2], shot[(y * W + x) * 3 + 1], shot[(y * W + x) * 3]];
const behindArrow = (f, x, y) => ease((f + 1) / CROSS) * (W + BAND + TIP) - (x + lag(y));

function arrive(shot) {
  const frames = [];
  for (let f = 0; f < CROSS; f++) {
    frames.push(
      frame((x, y) => {
        const d = behindArrow(f, x, y);
        if (d < 0) return CLEAR();
        const word = pixel(shot, x, y);
        if (d >= BAND) return [...word, 1];
        const k = 1 - d / BAND;
        return [ACCENT[0] * k + word[0] * (1 - k), ACCENT[1] * k + word[1] * (1 - k), ACCENT[2] * k + word[2] * (1 - k), 1];
      }),
    );
  }
  return frames; // the last frame, the word alone, stays until the element is replaced
}
function leave(shot) {
  const frames = [frame((x, y) => [...pixel(shot, x, y), 1])];
  for (let f = 0; f < CROSS; f++) {
    frames.push(
      frame((x, y) => {
        const d = behindArrow(f, x, y);
        if (d < 0) return [...pixel(shot, x, y), 1];
        if (d >= BAND) return CLEAR();
        return [...ACCENT, 1 - d / BAND];
      }),
    );
  }
  frames.push(frame(CLEAR));
  return frames;
}

// ---------- the word screens, as the bar draws them
const APP = "md_anim_gen";
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function capture(word) {
  const elements = [
    { id: "bg", type: "rectangle", display: "front", x: 0, y: 0, width: W, height: H, fill: "solid", fill_colors: ["#000000FF"], border_width: 0, timeout: 5 },
    { id: "title", type: "text", display: "front", text: word, font: "bold", x: 36, y: 3, align: "top_mid", color: "#FFFFFFFF", timeout: 5 },
  ];
  const drawn = await fetch(`${BAR}/api/display/draw`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ application_name: APP, priority: 90, elements }),
  });
  if (!drawn.ok) throw new Error(`draw ${word}: HTTP ${drawn.status}`);
  await sleep(400);
  // The body is base64 text: 72x16 pixels, blue, green, red, top row first.
  const shot = Buffer.from(await (await fetch(`${BAR}/api/screen?display=0`)).text(), "base64");
  if (shot.length !== W * H * 3) throw new Error(`capture ${word}: ${shot.length} bytes`);
  return shot;
}

mkdirSync(OUT, { recursive: true });
const files = [["reveal_up", reveal(true)], ["reveal_down", reveal(false)]];
try {
  for (const word of WORDS) {
    const shot = await capture(word);
    files.push([`in_${word.toLowerCase()}`, arrive(shot)], [`out_${word.toLowerCase()}`, leave(shot)]);
  }
} finally {
  await fetch(`${BAR}/api/display/draw?application_name=${APP}`, { method: "DELETE" }).catch(() => {});
}
let total = 0;
for (const [name, frames] of files) {
  const data = encode(frames);
  writeFileSync(join(OUT, `${name}.anim`), data);
  total += data.length;
  console.log(`${name}.anim  ${frames.length} frames  ${data.length} bytes`);
}
console.log(`${files.length} files, ${Math.round(total / 1024)} KiB`);
