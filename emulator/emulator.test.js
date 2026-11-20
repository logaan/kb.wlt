import assert from "node:assert/strict";
import test from "node:test";
import { inlineScene } from "./display.js";
import { keyBit, keyEdges, physicalCodes, positions } from "./keys.js";

test("physical punctuation and thumb keys occupy distinct Toucan positions", () => {
  assert.equal(positions.length, 42);
  assert.equal(physicalCodes.get("BracketLeft"), 11);
  assert.equal(physicalCodes.get("Slash"), 35);
  assert.equal(physicalCodes.get("BracketRight"), 36);
  assert.equal(physicalCodes.get("Backslash"), 41);
  assert.equal(positions[36].column, 3);
  assert.equal(positions[39].column, 0);
  assert.deepEqual(keyEdges(keyBit(1), keyBit(41), 27), [
    { position: 1, state: "released", timestampMs: 27 },
    { position: 41, state: "pressed", timestampMs: 27 },
  ]);
});

test("display scene encoding keeps coordinates, text byte lengths, and braille flags", () => {
  const scene = inlineScene([
    { opcode: 1, x: 258, y: 7, w: 270, h: 30, first: 5, second: 3, third: 0 },
    { opcode: 3, x: 265, y: 10, w: 0, h: 0, first: 1, second: 2, third: 1 },
    { opcode: 4, x: 12, y: 32, w: 28, h: 56, first: 128, second: 0x84, third: 0x42 },
  ], ["unused", "⢀"]);
  assert.deepEqual([...scene], [
    1, 2, 1, 7, 0, 14, 1, 30, 0, 5, 3,
    3, 9, 1, 10, 0, 1, 2, 3, 226, 162, 128,
    4, 12, 0, 32, 0, 28, 0, 56, 0, 128, 0x84, 0x42,
  ]);
});
