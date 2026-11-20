const rows = [
  [
    "Tab", "KeyQ", "KeyW", "KeyE", "KeyR", "KeyT",
    "KeyY", "KeyU", "KeyI", "KeyO", "KeyP", "BracketLeft",
  ],
  [
    "ControlLeft", "KeyA", "KeyS", "KeyD", "KeyF", "KeyG",
    "KeyH", "KeyJ", "KeyK", "KeyL", "Semicolon", "Quote",
  ],
  [
    "ShiftLeft", "KeyZ", "KeyX", "KeyC", "KeyV", "KeyB",
    "KeyN", "KeyM", "Comma", "Period", "Minus", "Slash",
  ],
  ["BracketRight", "Backspace", "MetaLeft", "Space", "Enter", "Backslash"],
];

const columnStagger = [37, 37, 12, 0, 12, 24, 24, 12, 0, 12, 37, 37];
const thumbs = [
  [350, 312, 0, 0, 0],
  [450, 312, 12, 450, 412],
  [548, 328, 24, 548, 433],
  [752, 328, -24, 852, 433],
  [850, 312, -12, 950, 412],
  [950, 312, 0, 0, 0],
];

function keyPlacement(row, index, left) {
  const [x, y, angle, pivotX, pivotY] = row === 3
    ? thumbs[index]
    : [(index + (left ? 0 : 2)) * 100, row * 100 + columnStagger[index], 0, 0, 0];
  const radians = angle * Math.PI / 180;
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);
  const dx = x + 50 - pivotX;
  const dy = y + 50 - pivotY;
  return {
    x: (pivotX + dx * cos - dy * sin) / 100 - (left ? 0 : 8),
    y: (pivotY + dx * sin + dy * cos) / 100,
    angle,
  };
}

export const positions = rows.flatMap((row, rowIndex) => row.map((code, index) => {
  const left = index < (rowIndex === 3 ? 3 : 6);
  const column = rowIndex === 3 ? (left ? index + 3 : index - 3) : index % 6;
  return { code, column, row: rowIndex, left, ...keyPlacement(rowIndex, index, left) };
}));

export const physicalCodes = new Map(positions.map(({ code }, position) => [code, position]));
physicalCodes.set("Escape", physicalCodes.get("BracketLeft"));
physicalCodes.set("ControlRight", physicalCodes.get("ControlLeft"));
physicalCodes.set("MetaRight", physicalCodes.get("MetaLeft"));
physicalCodes.set("ShiftRight", physicalCodes.get("Backslash"));
physicalCodes.set("NumpadEnter", physicalCodes.get("Enter"));

const printable = new Map([
  [40, "\n"], [43, "\t"], [44, " "], [45, "-_"], [46, "=+"], [47, "[{"],
  [48, "]}"], [49, "\\|"], [51, ";:"], [52, "'\""], [53, "`~"], [54, ",<"],
  [55, ".>"], [56, "/?"],
]);

const labels = new Map([
  [40, "Enter"], [41, "Esc"], [42, "Back"], [43, "Tab"], [44, "Space"],
  [57, "Caps"], [73, "Ins"], [74, "Home"], [75, "PgUp"], [76, "Del"],
  [77, "End"], [78, "PgDn"], [79, "→"], [80, "←"], [81, "↓"], [82, "↑"],
  [224, "Ctrl"], [225, "Shift"], [226, "Alt"], [227, "Meta"],
  [228, "Ctrl"], [229, "Shift"], [230, "Alt"], [231, "Meta"],
]);

export function character(usage, shifted) {
  if (usage >= 4 && usage <= 29) {
    return String.fromCharCode((shifted ? 65 : 97) + usage - 4);
  }
  if (usage >= 30 && usage <= 39) {
    return (shifted ? "!@#$%^&*()" : "1234567890")[usage - 30];
  }
  const pair = printable.get(usage);
  return pair?.[shifted && pair.length > 1 ? 1 : 0];
}

export function usageLabel(usage) {
  if (usage >= 58 && usage <= 69) return `F${usage - 57}`;
  return labels.get(usage) ?? character(usage, false)?.toUpperCase() ?? `0x${usage.toString(16)}`;
}

export function inputLabel(code) {
  if (code.startsWith("Key")) return code.slice(3);
  return ({
    BracketLeft: "[", BracketRight: "]", Backslash: "\\", Slash: "/",
    Minus: "-", Comma: ",", Period: ".", Semicolon: ";", Quote: "'",
    ControlLeft: "Left Ctrl", ShiftLeft: "Left Shift", MetaLeft: "Left Meta",
  })[code] ?? code;
}

export const keyBit = position => 1n << BigInt(position);

export function readBaseKeys(keyboard) {
  return positions.map((_, position) => {
    const output = keyboard.process(new Uint8Array(), {
      events: [{ position, state: "pressed", timestampMs: 0 }],
      nowMs: 0,
    });
    const [, actions] = keyboard.transformOutput(output.state, output.actions);
    return actions.find(({ tag }) => tag === "press" || tag === "tap")?.val.usage ?? 0;
  });
}

export function keyEdges(before, after, timestampMs) {
  const changed = before ^ after;
  return positions.flatMap((_, position) => {
    const bit = keyBit(position);
    if ((changed & bit) === 0n) return [];
    return [{ position, state: (after & bit) ? "pressed" : "released", timestampMs }];
  });
}
