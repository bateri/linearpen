// Draws linearpen's icon, its mark (an issue's circle with a tick in it, done), as PNGs, and writes
// them to src/assets/icon-<side>-<theme>.png. A terminal draws a program's icon only from the bytes
// it is given, so the program carries them (src/program/icons.rs names the files); run this again
// after changing the shapes here.
//
//   node tools/icon.mjs
//
// It uses nothing but Node: the shapes are strokes, filled with coverage taken from a grid of
// samples in each pixel, and the PNG is written by hand around zlib.

import { writeFileSync } from "node:fs";
import { deflateSync, crc32 } from "node:zlib";

/** The mark in its 12-unit box: a ring and a tick, each a stroke of this width, with an opacity. */
const WIDTH = 1.35;
const RING = { cx: 6, cy: 6, r: 4.3, opacity: 0.65 };
const TICK = { points: [[4, 6.2], [5.4, 7.6], [8.1, 4.6]], opacity: 1 };

/** The part of that box an icon shows: the mark (1.7 to 10.3) with a little around it. */
const FRAME = { x: 1, y: 1, side: 10 };

/** One colour for each ground, as an icon's `theme` says: the ground the icon is made to stand on. */
const COLOURS = {
  dark: [0xc5, 0x86, 0xc0],
  light: [0x8a, 0x3f, 0xa3],
};

/** 32 pixels is what a terminal's strip asks for on a Retina screen; 64 for those that draw larger. */
const SIDES = [32, 64];

const SAMPLES = 8;

/** How far the point is from the segment a-b. */
function segment(px, py, [ax, ay], [bx, by]) {
  const dx = bx - ax;
  const dy = by - ay;
  const t = Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)));
  return Math.hypot(px - (ax + t * dx), py - (ay + t * dy));
}

/** Whether the point is on the ring. */
function onRing(px, py) {
  return Math.abs(Math.hypot(px - RING.cx, py - RING.cy) - RING.r) <= WIDTH / 2;
}

/** Whether the point is on the tick. */
function onTick(px, py) {
  const [a, b, c] = TICK.points;
  return Math.min(segment(px, py, a, b), segment(px, py, b, c)) <= WIDTH / 2;
}

const SHAPES = [
  [onRing, RING.opacity],
  [onTick, TICK.opacity],
];

function draw(side, [red, green, blue]) {
  const scale = FRAME.side / side;
  const rows = [];
  for (let row = 0; row < side; row++) {
    const line = Buffer.alloc(1 + side * 4); // a leading 0: no filter
    for (let column = 0; column < side; column++) {
      // The shapes are drawn one over another, each with its opacity.
      let alpha = 0;
      for (const [inside, opacity] of SHAPES) {
        let hits = 0;
        for (let sy = 0; sy < SAMPLES; sy++) {
          for (let sx = 0; sx < SAMPLES; sx++) {
            const px = FRAME.x + (column + (sx + 0.5) / SAMPLES) * scale;
            const py = FRAME.y + (row + (sy + 0.5) / SAMPLES) * scale;
            if (inside(px, py)) hits++;
          }
        }
        const cover = (hits / (SAMPLES * SAMPLES)) * opacity;
        alpha = cover + alpha * (1 - cover);
      }
      line.set([red, green, blue, Math.round(alpha * 255)], 1 + column * 4);
    }
    rows.push(line);
  }
  return png(side, Buffer.concat(rows));
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, crc]);
}

function png(side, pixels) {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(side, 0);
  header.writeUInt32BE(side, 4);
  header.set([8, 6, 0, 0, 0], 8); // 8 bits, RGBA, deflate, no filter method, no interlace
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(pixels, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

let written = 0;
for (const [theme, colour] of Object.entries(COLOURS)) {
  for (const side of SIDES) {
    writeFileSync(
      new URL(`../src/assets/icon-${side}-${theme}.png`, import.meta.url),
      draw(side, colour),
    );
    written++;
  }
}
console.log(`wrote ${written} icons to src/assets`);
