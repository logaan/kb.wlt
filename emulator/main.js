import { makeDisplay } from "./display.js";
import { character, inputLabel, keyBit, keyEdges, physicalCodes, positions, readBaseKeys, usageLabel } from "./keys.js";

const element = id => document.getElementById(id);
const runtime = element("runtime");
const output = element("text-output");
const lastReport = element("last-report");
const mode = element("mode");
const viewer = element("display-viewer");
const enlargedDisplay = element("enlarged-display");
const usbLink = element("usb-link");
const bluetoothLink = element("bluetooth-link");
const usbTransport = element("transport-usb");
const bluetoothTransport = element("transport-bluetooth");
const leftLink = element("left-link");
const rightLink = element("right-link");
const leftBattery = element("left-battery");
const rightBattery = element("right-battery");
const decoder = new TextDecoder();
const storageKey = "kb.wlt.toucan.settings";
const transportKey = "kb.wlt.toucan.transport";
const actionTags = { press: 0, release: 1, tap: 2, "release-all": 3 };
const start = performance.now();

const keycaps = positions.map(key => {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "key";
  button.style.gridRow = key.row + 1;
  button.style.gridColumn = key.column + 1;
  button.disabled = true;
  if (key.row === 3 || key.column === (key.left ? 0 : 5)) button.dataset.alternate = "";
  element(key.left ? "left-keys" : "right-keys").append(button);
  return button;
});

function makeDotCell(dot) {
  const cell = document.createElement("span");
  cell.className = "dot-cell";
  cell.setAttribute("aria-hidden", "true");
  for (const position of [1, 2, 3, 7, 4, 5, 6, 8]) {
    const mark = document.createElement("span");
    if (position === dot) mark.className = "active-dot";
    cell.append(mark);
  }
  return cell;
}

function updateLegends(braille, thumb, base, dots) {
  mode.textContent = braille ? "Braille" : "QWERTY";
  element("reference-help").hidden = !braille;
  element("viewer-reference-help").hidden = !braille;
  for (const [position, keycap] of keycaps.entries()) {
    const mask = braille ? dots[position] : 0;
    const dot = mask && mask < 256 ? Math.log2(mask) + 1 : 0;
    const input = inputLabel(positions[position].code);
    const toggle = position === thumb;
    const label = toggle ? "BRL" : mask === 256 ? "Space" : base[position] && !braille ? usageLabel(base[position]) : "";
    const description = dot ? String.fromCodePoint(0x2800 | mask) : toggle ? "Braille layer" : label || "Unassigned";
    keycap.toggleAttribute("data-dot", dot !== 0);
    keycap.toggleAttribute("data-layer-switch", toggle);
    keycap.setAttribute("aria-label", `${description} (${input})`);
    keycap.replaceChildren(dot ? makeDotCell(dot) : label);
  }
}

function replaceOutput(usage, modifiers) {
  if (modifiers & 0xdd) return;
  const { value, selectionStart, selectionEnd } = output;
  const start = document.activeElement === output ? selectionStart : value.length;
  const end = document.activeElement === output ? selectionEnd : value.length;
  const text = character(usage, Boolean(modifiers & 0x22));
  if (text !== undefined) {
    output.setRangeText(text, start, end, "end");
  } else if (usage === 42) {
    const removeFrom = start === end ? Math.max(start - 1, 0) : start;
    output.setRangeText("", removeFrom, end, "end");
  } else if (usage === 76) {
    output.setRangeText("", start, start === end ? Math.min(end + 1, value.length) : end, "start");
  } else if (usage === 79 || usage === 80) {
    const next = usage === 79 ? Math.min(end + 1, value.length) : Math.max(start - 1, 0);
    output.setSelectionRange(next, next);
  }
  if (document.activeElement !== output) output.scrollTop = output.scrollHeight;
}

function reportSink() {
  let usages = [];
  let modifiers = 0;
  let consumer = 0;
  let keyboard = new Uint8Array(8);

  function updateReport() {
    const hex = Array.from(keyboard, byte => byte.toString(16).padStart(2, "0")).join(" ");
    lastReport.value = consumer ? `${hex} · Consumer ${consumer.toString(16)}` : hex;
  }

  return {
    keyboard_report(report) {
      const bytes = new Uint8Array(8);
      new DataView(bytes.buffer).setBigUint64(0, report, true);
      modifiers = bytes[0];
      const next = [...bytes.subarray(2)].filter(usage => usage > 3);
      for (const usage of next) {
        if (!usages.includes(usage)) replaceOutput(usage, modifiers);
      }
      keyboard = bytes;
      usages = next;
      updateReport();
    },

    consumer_report(report) {
      consumer = report;
      updateReport();
    },

    repeat(usage) {
      if (usages.includes(usage)) replaceOutput(usage, modifiers);
    },
  };
}

async function loadCore(path, imports) {
  const response = await fetch(new URL(`./components/${path}`, import.meta.url));
  if (!response.ok) throw new Error(`Missing build output ${path}`);
  return (await WebAssembly.instantiateStreaming(response, imports)).instance.exports;
}


function stepResult(output) {
  const kind = output.kind === "sticky" ? 1n << 31n : 0n;
  const packed = output.kind === "idle" ? 0n
    : kind | BigInt(output.consumer) << 16n | BigInt(output.modifiers) << 8n | BigInt(output.usage);
  return packed << 32n | BigInt(output.state.chord) << 16n | BigInt(output.state.held);
}

function storedSettings(adapter) {
  return new Uint8Array(adapter.memory.buffer, adapter.settings_buffer(), adapter.settings_len());
}

function loadSettings(adapter) {
  try {
    const value = localStorage.getItem(storageKey);
    if (value) {
      const bytes = Uint8Array.from(atob(value), byte => byte.charCodeAt(0));
      if (bytes.length === adapter.settings_len()) {
        storedSettings(adapter).set(bytes);
        adapter.load_settings();
      }
    }
  } catch (error) {
    console.warn("Toucan settings could not be loaded", error);
  }
}

function saveSettings(adapter) {
  try {
    localStorage.setItem(storageKey, btoa(String.fromCharCode(...storedSettings(adapter))));
  } catch (error) {
    console.warn("Toucan settings could not be saved", error);
  }
}

function batteryValue(field) {
  return field.value === "" ? 255 : Math.min(100, Math.max(0, field.valueAsNumber));
}

async function run() {
  const { instantiate } = await import("./components/toucan.js");
  const modules = new Map();
  function loadModule(path) {
    if (!modules.has(path)) {
      const url = new URL(`./components/${path}`, import.meta.url);
      modules.set(path, WebAssembly.compileStreaming(fetch(url)));
    }
    return modules.get(path);
  }
  const [receiver, left] = await Promise.all([instantiate(loadModule, {}), instantiate(loadModule, {})]);
  const { keyboard, braille, display } = left;
  const sink = reportSink();
  const adapter = await loadCore("toucan-adapter.wasm", {
    host: {
      guest_step: (held, chord, keys) => stepResult(braille.step({ held, chord }, keys)),
      guest_map_cell: braille.mapCell,
      guest_navigate: display.navigate,
      keyboard_report: sink.keyboard_report,
      consumer_report: sink.consumer_report,
    },
  });

  loadSettings(adapter);
  const drawProspector = makeDisplay(element("prospector"), 0, adapter);
  const drawNiceView = makeDisplay(element("nice-view"), 1, adapter);
  const baseKeys = readBaseKeys(keyboard);
  const thumb = adapter.braille_thumb();
  const dots = positions.map((_, position) => adapter.braille_mask(keyBit(position)));
  const inputs = new Map();
  let componentState = new Uint8Array();
  let previousKeys = 0n;
  let brailleActive = false;
  let previousNiceView = "";
  let previousProspector = "";
  let direct;
  let displayedCanvas;
  let previewButton;
  let wakeAt;
  let renderQueued = false;
  let timer;
  let failed = false;

  function drawEnlargedDisplay() {
    if (viewer.hidden) return;
    enlargedDisplay.getContext("2d").drawImage(displayedCanvas, 0, 0);
  }

  function closeDisplay() {
    viewer.hidden = true;
    previewButton.setAttribute("aria-expanded", "false");
    previewButton.focus({ preventScroll: true });
  }

  function openDisplay(button, id, title) {
    if (!viewer.hidden && previewButton === button) {
      closeDisplay();
      return;
    }
    previewButton?.setAttribute("aria-expanded", "false");
    previewButton = button;
    previewButton.setAttribute("aria-expanded", "true");
    displayedCanvas = element(id);
    enlargedDisplay.width = displayedCanvas.width;
    enlargedDisplay.height = displayedCanvas.height;
    viewer.style.setProperty("--zoomed-width", `${displayedCanvas.width * 2}px`);
    element("display-title").textContent = title;
    viewer.hidden = false;
    drawEnlargedDisplay();
    enlargedDisplay.focus({ preventScroll: true });
    viewer.scrollIntoView({ block: "nearest" });
  }

  function hostReady() {
    const connection = direct ? bluetoothLink : usbLink;
    return leftLink.checked && connection.checked;
  }

  function readModel() {
    const model = new Uint8Array(adapter.memory.buffer, adapter.model_bytes(), adapter.model_len()).slice();
    const detail = decoder.decode(new Uint8Array(adapter.memory.buffer, adapter.model_detail(), adapter.model_detail_len()));
    return { model, detail, key: Array.from(model).join(",") + "|" + detail };
  }

  function render() {
    renderQueued = false;
    const links = Number(leftLink.checked) | Number(rightLink.checked && leftLink.checked) << 1 | Number(hostReady()) << 2;
    adapter.update_model(links, batteryValue(leftBattery), batteryValue(rightBattery), direct, false);
    const niceView = readModel();
    if (niceView.key !== previousNiceView) {
      drawNiceView(display, niceView.model, niceView.detail);
      previousNiceView = niceView.key;
    }
    if (direct || !leftLink.checked) {
      adapter.update_model(Number(usbLink.checked) << 2, 255, 255, false, true);
    }
    const prospector = readModel();
    if (prospector.key !== previousProspector) {
      drawProspector(receiver.display, prospector.model, prospector.detail);
      previousProspector = prospector.key;
    }
    drawEnlargedDisplay();
  }

  function safely(callback) {
    if (failed) return;
    try {
      callback();
    } catch (error) {
      failed = true;
      clearInterval(timer);
      reportFailure(error);
    }
  }

  function updateDisplay() {
    if (renderQueued) return;
    renderQueued = true;
    requestAnimationFrame(() => safely(render));
  }

  function update(ready = hostReady()) {
    const nowMs = (performance.now() - start) >>> 0;
    let matrix = 0n;
    for (const position of inputs.values()) {
      if (positions[position].left ? leftLink.checked : rightLink.checked) matrix |= keyBit(position);
    }
    const route = adapter.route_update(matrix, nowMs);
    if (route === 2) selectTransport(direct ? "usb" : "bluetooth", false);
    ready &&= route === 0;
    const keys = adapter.matrix(matrix, nowMs, ready);
    for (const [position, keycap] of keycaps.entries()) {
      keycap.toggleAttribute("data-pressed", Boolean(matrix & keyBit(position)));
    }

    const nextLayer = Boolean(adapter.braille_active());
    if (nextLayer !== brailleActive) {
      componentState = new Uint8Array();
      previousKeys = 0n;
      wakeAt = undefined;
      brailleActive = nextLayer;
      updateLegends(brailleActive, thumb, baseKeys, dots);
    }

    if (brailleActive) {
      if (ready && adapter.braille_update(adapter.braille_mask(keys), nowMs)) saveSettings(adapter);
    } else if (keys !== previousKeys || (wakeAt !== undefined && ((nowMs - wakeAt) | 0) >= 0)) {
      const result = keyboard.process(componentState, { events: keyEdges(previousKeys, keys, nowMs), nowMs });
      const [state, actions] = keyboard.transformOutput(result.state, result.actions);
      componentState = state;
      wakeAt = result.wakeAtMs;
      for (const { tag, val } of actions) adapter.key_action(actionTags[tag], val?.usage ?? 0, val?.modifiers ?? 0);
    }
    previousKeys = keys;
    updateDisplay();
  }

  function press(source, position) {
    if (inputs.has(source)) return;
    inputs.set(source, position);
    safely(update);
  }

  function release(source) {
    if (inputs.delete(source)) safely(update);
  }

  function cancel() {
    inputs.clear();
    safely(() => update(false));
  }

  function selectTransport(value, cancelInput = true) {
    const nextDirect = value === "bluetooth";
    if (cancelInput && direct !== nextDirect) cancel();
    direct = nextDirect;
    usbTransport.checked = !nextDirect;
    bluetoothTransport.checked = nextDirect;
    element("output-heading").textContent = nextDirect ? "Bluetooth output" : "USB output";
    try {
      localStorage.setItem(transportKey, nextDirect ? "bluetooth" : "usb");
    } catch (error) {
      console.warn("Toucan transport could not be saved", error);
    }
    updateDisplay();
  }

  try {
    selectTransport(localStorage.getItem(transportKey));
  } catch (error) {
    console.warn("Toucan transport could not be loaded", error);
    selectTransport("usb");
  }

  updateLegends(false, thumb, baseKeys, dots);
  keycaps.forEach((keycap, position) => {
    keycap.disabled = false;
    keycap.addEventListener("pointerdown", event => {
      if (event.button !== 0) return;
      event.preventDefault();
      keycap.setPointerCapture(event.pointerId);
      press(`pointer:${event.pointerId}`, position);
    });
    for (const type of ["pointerup", "lostpointercapture"]) {
      keycap.addEventListener(type, event => release(`pointer:${event.pointerId}`));
    }
    keycap.addEventListener("pointercancel", cancel);
    keycap.addEventListener("click", event => {
      if (event.detail !== 0) return;
      const source = `button:${position}`;
      press(source, position);
      release(source);
    });
  });

  window.addEventListener("keydown", event => {
    if (!viewer.hidden && viewer.contains(event.target) && event.code === "Escape") {
      event.preventDefault();
      closeDisplay();
      return;
    }
    const position = physicalCodes.get(event.code);
    if (position === undefined || event.isComposing || event.target.closest?.(".links")) return;
    const nativeControl = event.target.closest?.(".clear, .screen-toggle, .transport");
    if (nativeControl && ["Enter", "Space", "Tab"].includes(event.code)) return;
    if (event.code === "Tab" && event.shiftKey) return;
    event.preventDefault();
    if (event.repeat) {
      if (!brailleActive) sink.repeat(baseKeys[position]);
    } else {
      press(event.code, position);
    }
  });
  window.addEventListener("keyup", event => {
    if (inputs.has(event.code)) {
      event.preventDefault();
      release(event.code);
    }
  });
  window.addEventListener("blur", () => {
    if (inputs.size) cancel();
  });
  document.addEventListener("visibilitychange", () => {
    if (document.hidden && inputs.size) cancel();
  });

  for (const input of [usbTransport, bluetoothTransport]) {
    input.addEventListener("change", () => selectTransport(input.value));
  }
  for (const field of [leftLink, rightLink, usbLink, bluetoothLink]) {
    field.addEventListener("change", () => {
      document.querySelector(".board-left").toggleAttribute("data-disconnected", !leftLink.checked);
      document.querySelector(".board-right").toggleAttribute("data-disconnected", !rightLink.checked);
      if (field === leftLink || field === rightLink || field === (direct ? bluetoothLink : usbLink)) {
        cancel();
      } else {
        updateDisplay();
      }
    });
  }
  for (const field of [leftBattery, rightBattery]) field.addEventListener("input", updateDisplay);
  element("zoom-prospector").addEventListener("click", event => openDisplay(event.currentTarget, "prospector", "Prospector"));
  element("zoom-nice-view").addEventListener("click", event => openDisplay(event.currentTarget, "nice-view", "Toucan nice!view"));
  element("close-viewer").addEventListener("click", closeDisplay);
  element("clear").addEventListener("click", () => {
    output.value = "";
    output.focus();
  });

  render();
  sink.keyboard_report(0n);
  runtime.dataset.status = "ready";
  runtime.textContent = "Components ready";
  timer = setInterval(() => {
    if (inputs.size || brailleActive || wakeAt !== undefined) safely(update);
  }, 32);
}

function reportFailure(error) {
  runtime.dataset.status = "error";
  runtime.textContent = `Emulator stopped. ${error.message}`;
  for (const tip of keycaps) tip.disabled = true;
  console.error(error);
}

run().catch(reportFailure);
