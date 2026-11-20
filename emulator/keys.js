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

export const positions = rows.flatMap((row, rowIndex) => row.map((code, index) => {
  const left = index < (rowIndex === 3 ? 3 : 6);
  const column = rowIndex === 3 ? (left ? index + 3 : index - 3) : index % 6;
  return { code, column, row: rowIndex, left };
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
