import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const entrypoint = fileURLToPath(new URL("./serve.mjs", import.meta.url));
const server = spawn(process.execPath, [entrypoint], {
  env: { ...process.env, PORT: "0" },
  stdio: ["ignore", "pipe", "inherit"],
});
let browser;
try {
  const [startup] = await Promise.race([
    once(server.stdout, "data"),
    once(server, "exit").then(([code]) => {
      throw new Error(`Emulator server exited with status ${code}`);
    }),
  ]);
  const url = startup.toString().trim().split(" ").at(-1);
  browser = await chromium.launch({ channel: "chrome" });
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  page.on("console", message => {
    if (message.type() === "error" && !message.text().includes("404")) {
      errors.push(message.text());
    }
  });
  await page.goto(url);
  await page.locator("#runtime[data-status]").waitFor();
  assert.equal(await page.locator("#runtime").textContent(), "Components ready", errors.join("\n"));
  assert.equal(await page.locator(".key").count(), 42);
  assert.equal(await page.getByRole("radio").count(), 0);
  assert.equal(await page.locator("#route").textContent(), "Prospector");
  assert.equal(await page.locator("#mode").textContent(), "QWERTY");
  assert.equal(await page.getByRole("button", { name: "Esc ([)" }).count(), 1);
  assert.equal(await page.getByRole("button", { name: "- (/)" }).count(), 1);

  const canvas = id => page.locator(id).evaluate(canvas =>
    canvas.toDataURL());
  async function checkCorners(selector, rounded) {
    const radius = await page.locator(selector).evaluate(element => getComputedStyle(element).borderTopLeftRadius);
    if (rounded) {
      const radii = radius.split(" ").map(Number.parseFloat);
      assert.ok(Math.abs(radii[0] * 280 / 100 - 30) < 0.01);
      assert.ok(Math.abs(radii[1] * 240 / 100 - 30) < 0.01);
    } else {
      assert.equal(radius, "0px");
    }
  }
  await checkCorners("#prospector", true);
  await checkCorners("#toucan-display", false);
  const initialProspector = await canvas("#prospector");
  const initialToucanDisplay = await canvas("#toucan-display");
  assert.notEqual(initialProspector, initialToucanDisplay);

  const output = () => page.locator("#text-output").inputValue();
  await page.keyboard.press("KeyQ");
  await page.keyboard.press("ShiftLeft+KeyW");
  await page.keyboard.press("Space");
  await page.keyboard.press("KeyE");
  await page.keyboard.press("Slash");
  await page.keyboard.press("BracketLeft");
  await page.keyboard.press("Tab");
  await page.keyboard.press("Enter");
  assert.equal(await output(), "qW e-\t\n");
  await page.getByRole("button", { name: "P (P)", exact: true }).click();
  assert.equal(await output(), "qW e-\t\np");

  await page.keyboard.press("BracketRight");
  assert.equal(await page.locator("#mode").textContent(), "Braille");
  assert.equal(await page.locator(".dot-cell").count(), 16);
  const brailleLegends = [
    ["⡀ (A)", "⠄ (S)", "⠂ (D)", "⠁ (F)"],
    ["⢀ (Z)", "⠠ (X)", "⠐ (C)", "⠈ (V)"],
    ["⠈ (J)", "⠐ (K)", "⠠ (L)", "⢀ (;)"],
    ["⠁ (M)", "⠂ (,)", "⠄ (.)", "⡀ (-)"],
  ];
  for (const legend of brailleLegends.flat()) {
    assert.equal(await page.getByRole("button", { name: legend, exact: true }).count(), 1, legend);
  }
  assert.equal(await page.getByRole("button", { name: "Space (Left Meta)", exact: true }).count(), 1);
  assert.equal(await page.getByRole("button", { name: "Space (Space)", exact: true }).count(), 1);
  await page.keyboard.down("KeyF");
  await page.waitForTimeout(50);
  assert.ok((await canvas("#prospector")) !== initialProspector, (await page.locator("#runtime").textContent()) + " " + errors.join(" / "));
  assert.notEqual(await canvas("#toucan-display"), initialToucanDisplay);
  await page.keyboard.up("KeyF");
  assert.equal(await output(), "qW e-\t\npa");
  await page.keyboard.press("KeyM");
  assert.equal(await output(), "qW e-\t\npaa");
  await page.keyboard.press("Minus");
  assert.equal(await output(), "qW e-\t\npa");

  async function chord(...codes) {
    for (const code of codes) await page.keyboard.down(code);
    for (const code of codes.toReversed()) await page.keyboard.up(code);
    await page.waitForTimeout(40);
  }
  const brailleHome = await canvas("#prospector");
  await chord("Space", "KeyF", "KeyD", "KeyS", "KeyV", "KeyC");
  const namespace = await canvas("#prospector");
  assert.notEqual(namespace, brailleHome);
  await chord("Space", "KeyF", "KeyD", "KeyS", "KeyC");
  assert.notEqual(await canvas("#prospector"), namespace);
  const categories = await canvas("#prospector");
  await page.getByRole("button", { name: "Enlarge Prospector display" }).click();
  assert.equal(await page.locator("#display-viewer").isVisible(), true);
  await checkCorners("#enlarged-display", true);
  assert.equal(await page.locator("dialog, [title], [popover]").count(), 0);
  const before = await canvas("#enlarged-display");
  await chord("Space", "KeyA");
  assert.notEqual(await canvas("#prospector"), categories);
  assert.notEqual(await canvas("#enlarged-display"), before);
  await page.keyboard.press("Escape");
  assert.equal(await page.locator("#display-viewer").isVisible(), false);
  await page.getByRole("button", { name: "Enlarge Toucan display" }).click();
  await checkCorners("#enlarged-display", false);
  await page.getByRole("button", { name: "Close", exact: true }).click();
  assert.equal(await output(), "qW e-\t\npa");

  await chord("KeyF", "KeyD", "KeyC");
  await chord("KeyF", "KeyS", "KeyV", "KeyC");
  await chord("KeyF", "KeyS", "KeyV", "KeyC");
  const lastNavigationHelp = await canvas("#prospector");
  await chord("KeyF", "KeyS", "KeyV", "KeyC");
  assert.notEqual(await canvas("#prospector"), lastNavigationHelp);
  assert.equal(await page.locator("#route").textContent(), "Prospector");

  await page.keyboard.press("BracketRight");
  assert.equal(await page.locator("#mode").textContent(), "QWERTY");
  await page.keyboard.down("BracketRight");
  await page.waitForTimeout(240);
  assert.equal(await page.locator("#mode").textContent(), "Braille");
  await chord("KeyF");
  await page.keyboard.up("BracketRight");
  assert.equal(await page.locator("#mode").textContent(), "QWERTY");
  assert.equal(await output(), "qW e-\t\npaa");
  await page.keyboard.down("BracketRight");
  await page.waitForTimeout(240);
  await page.keyboard.down("KeyF");
  await page.keyboard.up("BracketRight");
  await page.keyboard.up("KeyF");
  assert.equal(await page.locator("#mode").textContent(), "QWERTY");
  assert.equal(await output(), "qW e-\t\npaa");

  await page.locator(".links summary").click();
  await page.locator("#left-battery").fill("20");
  await page.waitForTimeout(40);
  assert.ok((await canvas("#prospector")) !== initialProspector, (await page.locator("#runtime").textContent()) + " " + errors.join(" / "));
  await page.locator("#right-link").uncheck();
  await page.locator("#text-output").focus();
  const beforeDisconnected = await output();
  await page.keyboard.press("KeyP");
  assert.equal(await output(), beforeDisconnected);
  await page.keyboard.press("KeyQ");
  assert.equal(await output(), beforeDisconnected + "q");
  await page.locator("#right-link").check();
  await page.locator("#text-output").focus();

  async function pickTarget(previous, next, side = "left") {
    const dots = side === "left"
      ? ["KeyF", "KeyD", "KeyS", "KeyV", "KeyC", "KeyX", "KeyA", "KeyZ"]
      : ["KeyM", "Comma", "Period", "KeyJ", "KeyK", "KeyL", "Minus", "Semicolon"];
    const space = side === "left" ? "MetaLeft" : "Space";
    await chord(space, ...dots.slice(0, 5));
    await chord(...dots.slice(1, 5));
    for (let row = Math.min(previous, next); row < Math.max(previous, next); row += 1) {
      await chord(space, dots[previous < next ? 5 : 2]);
    }
    await chord(space, dots[6]);
  }

  await page.locator("#usb-link").uncheck();
  await page.locator("#prospector-link").uncheck();
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
  await page.locator("#text-output").focus();
  const beforeSwitch = await output();
  const disconnectedProspector = await canvas("#prospector");
  await page.keyboard.press("KeyP");
  assert.equal(await output(), beforeSwitch);
  await page.keyboard.press("BracketRight");
  await pickTarget(0, 1, "left");
  assert.equal(await page.locator("#route").textContent(), "BT 1");
  assert.equal(await page.locator("#output-heading").textContent(), "Bluetooth output");
  assert.equal(await output(), beforeSwitch);
  await page.keyboard.press("BracketRight");

  await page.keyboard.press("KeyP");
  assert.equal(await output(), beforeSwitch + "p");
  await page.keyboard.press("BracketRight");
  const bluetoothHome = await canvas("#toucan-display");
  await page.keyboard.down("KeyF");
  await page.waitForTimeout(50);
  assert.notEqual(await canvas("#toucan-display"), bluetoothHome);
  await page.keyboard.up("KeyF");
  assert.equal(await output(), beforeSwitch + "pa");
  assert.equal(await canvas("#prospector"), disconnectedProspector);
  await page.locator("#prospector-link").check();
  await page.waitForFunction(before => document.getElementById("prospector").toDataURL() !== before, disconnectedProspector);
  await page.reload();
  await page.locator('#runtime[data-status="ready"]').waitFor();
  assert.equal(await page.locator("#route").textContent(), "BT 1");

  await page.keyboard.press("BracketRight");
  await chord("Space", "KeyF", "KeyD", "KeyS", "KeyV", "KeyC");
  await chord("Space", "KeyF", "KeyD", "KeyS", "KeyV", "KeyC");
  await chord("Space", "KeyV");
  await chord("Space", "KeyV");
  await chord("Space", "KeyA");
  await chord("Space", "KeyA");
  await chord("Space", "KeyA");
  await chord("KeyF");
  await chord("KeyF", "KeyD");
  await chord("KeyF", "KeyV");
  await chord("KeyA");
  await chord("Space", "KeyA");
  assert.ok(await page.evaluate(() => localStorage.getItem("kb.wlt.toucan.settings")));
  assert.equal(await output(), "");
  await page.keyboard.press("BracketRight");
  await page.keyboard.press("BracketRight");
  await chord("KeyF");
  assert.equal(await output(), "b");

  await page.keyboard.press("BracketRight");
  await page.locator(".links summary").click();
  await page.locator("#bluetooth-link").uncheck();
  await page.locator("#text-output").focus();
  await page.keyboard.press("KeyP");
  assert.equal(await output(), "b");
  await page.keyboard.press("Backslash");
  assert.equal(await page.locator("#mode").textContent(), "Braille");
  for (let slot = 2; slot <= 5; slot += 1) {
    await pickTarget(slot - 1, slot, "right");
    assert.equal(await page.locator("#route").textContent(), "BT " + slot);
    assert.equal(await output(), "b");
  }
  await pickTarget(5, 6, "right");
  assert.equal(await page.locator("#route").textContent(), "USB");
  assert.equal(await page.locator("#output-heading").textContent(), "USB output");
  assert.equal(await output(), "b");
  await page.keyboard.press("Backslash");
  await page.locator("#toucan-usb-link").uncheck();
  await page.locator("#text-output").focus();
  await page.keyboard.press("KeyP");
  assert.equal(await output(), "b");
  await page.locator("#toucan-usb-link").check();
  await page.locator("#text-output").focus();
  await page.keyboard.press("KeyP");
  assert.equal(await output(), "bp");
  await page.keyboard.press("Backslash");
  await pickTarget(6, 0, "right");
  assert.equal(await page.locator("#route").textContent(), "Prospector");
  await page.evaluate(() => localStorage.removeItem("kb.wlt.toucan.settings"));
  await page.reload();
  await page.locator('#runtime[data-status="ready"]').waitFor();
  assert.equal(await page.locator("#route").textContent(), "Prospector");
  const mouseHome = await canvas("#prospector");
  await page.locator("#prospector").click();
  await page.waitForFunction(home => document.getElementById("prospector").toDataURL() !== home, mouseHome);
  const mouseMenu = await canvas("#prospector");
  const panel = await page.locator("#prospector").boundingBox();
  await page.mouse.move(panel.x + panel.width / 2, panel.y + panel.height / 2);
  await page.mouse.down();
  await page.mouse.move(panel.x + panel.width / 2, panel.y + panel.height / 2 + 40);
  await page.mouse.up();
  await page.waitForFunction(menu => document.getElementById("prospector").toDataURL() !== menu, mouseMenu);
  assert.equal(await page.locator("#display-viewer").isVisible(), false);
  await page.locator("#prospector").click({ delay: 580 });
  await page.waitForFunction(home => document.getElementById("prospector").toDataURL() === home, mouseHome);

  async function checkGeometry(page) {
    const geometry = await page.evaluate(() => {
      const rect = id => document.querySelector(id).getBoundingClientRect();
      const centerX = box => (box.left + box.right) / 2;
      const centerY = box => (box.top + box.bottom) / 2;
      const leftKeys = [...document.querySelectorAll("#left-keys .key")];
      const rightKeys = [...document.querySelectorAll("#right-keys .key")];
      const leftBoard = rect(".board-left");
      const rightBoard = rect(".board-right");
      const workbench = rect(".workbench");
      const receiver = rect(".receiver");
      const mipFrame = rect(".toucan-display-case");
      const [tab, q] = leftKeys.map(key => key.getBoundingClientRect());
      const ctrl = leftKeys[6].getBoundingClientRect();
      const dx = q.left - tab.left;
      const dy = ctrl.top - tab.top;
      const angles = keys => keys.map(key => {
        const matrix = new DOMMatrixReadOnly(getComputedStyle(key).transform);
        return Math.round(Math.atan2(matrix.b, matrix.a) * 180 / Math.PI) + 0;
      });
      const topOffsets = keys => {
        const top = Math.min(...keys.map(key => key.getBoundingClientRect().top));
        return keys.map(key => (key.getBoundingClientRect().top - top) / dy);
      };
      const thumbs = [...leftKeys.slice(-3), ...rightKeys.slice(-3)];
      const keyBounds = [leftKeys, rightKeys].flatMap((keys, index) => keys.map(key => {
        const box = key.getBoundingClientRect();
        const board = index === 0 ? leftBoard : rightBoard;
        return box.left >= board.left && box.right <= board.right &&
          box.top >= board.top && box.bottom <= board.bottom &&
          (box.right <= mipFrame.left || box.left >= mipFrame.right ||
            box.top >= mipFrame.bottom || box.bottom <= mipFrame.top) &&
          document.elementFromPoint(centerX(box), centerY(box)) === key;
      }));
      return {
        pitch: dx / dy,
        offsets: [...topOffsets(leftKeys.slice(0, 6)), ...topOffsets(rightKeys.slice(0, 6))],
        angles: angles([...leftKeys.slice(0, 18), ...rightKeys.slice(0, 18), ...thumbs]),
        thumbDepths: thumbs.map(key => (centerY(key.getBoundingClientRect()) - ctrl.top) / dy),
        receiverOffset: centerX(receiver) - (leftBoard.right + rightBoard.left) / 2,
        receiverGap: Math.min(leftBoard.top, rightBoard.top) - receiver.bottom,
        alignedBoards: leftBoard.top === rightBoard.top,
        attachedMip: Boolean(document.querySelector(".board-left > .toucan-display-case")),
        fittingBoards: leftBoard.left >= workbench.left && rightBoard.right <= workbench.right,
        fittingKeys: keyBounds.every(Boolean),
        transforms: [".board-left", ".board-right"].map(selector => getComputedStyle(document.querySelector(selector)).transform),
        overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth,
      };
    });
    assert.ok(Math.abs(geometry.pitch - 18 / 17) < 0.002);
    for (const [index, expected] of [0.37, 0.37, 0.12, 0, 0.12, 0.24, 0.24, 0.12, 0, 0.12, 0.37, 0.37].entries()) {
      assert.ok(Math.abs(geometry.offsets[index] - expected) < 0.002);
    }
    for (const [index, expected] of [2.221, 2.336, 2.632, 2.632, 2.336, 2.221].entries()) {
      assert.ok(Math.abs(geometry.thumbDepths[index] - expected) < 0.003);
    }
    assert.deepEqual(geometry.angles, [...Array(36).fill(0), 0, 12, 24, -24, -12, 0]);
    assert.ok(Math.abs(geometry.receiverOffset) < 0.5);
    assert.ok(geometry.receiverGap >= 20);
    assert.ok(geometry.alignedBoards && geometry.fittingBoards && geometry.fittingKeys && geometry.attachedMip);
    assert.deepEqual(geometry.transforms, ["none", "none"]);
    assert.equal(geometry.overflow, false);
  }

  async function checkDeviceScale(page, selector, expectedScale) {
    const { native, pixels, origin } = await page.locator(selector).evaluate(canvas => {
      const { left, top, width, height } = canvas.getBoundingClientRect();
      const ratio = devicePixelRatio;
      return {
        native: [canvas.width, canvas.height],
        pixels: [width * ratio, height * ratio],
        origin: [left * ratio, top * ratio],
      };
    });
    assert.ok(pixels.every((size, axis) => Math.abs(size - native[axis] * expectedScale) < 0.05));
    assert.ok(origin.every(position => Math.abs(position - Math.round(position)) < 0.05));
  }

  await checkGeometry(page);
  await checkDeviceScale(page, "#prospector", 1);
  await checkDeviceScale(page, "#toucan-display", 1);

  const touch = await browser.newPage({ viewport: { width: 1024, height: 768 }, hasTouch: true, isMobile: true });
  await touch.goto(url);
  await touch.locator('#runtime[data-status="ready"]').waitFor();
  await checkGeometry(touch);
  const leftOuter = touch.getByRole("button", { name: "Braille layer (])", exact: true });
  const rightOuter = touch.getByRole("button", { name: "Braille layer (\\)", exact: true });
  await leftOuter.tap();
  assert.equal(await touch.locator("#mode").textContent(), "Braille");
  await rightOuter.tap();
  assert.equal(await touch.locator("#mode").textContent(), "QWERTY");
  assert.equal(await touch.locator("#text-output").inputValue(), "");
  const touchPanel = touch.locator("#prospector");
  const touchCanvas = () => touchPanel.evaluate(canvas => canvas.toDataURL());
  const linkedHome = await touchCanvas();
  await touchPanel.tap();
  await touch.waitForFunction(home => document.getElementById("prospector").toDataURL() !== home, linkedHome);
  assert.equal(await touch.locator("#display-viewer").isVisible(), false);
  const linkedMenu = await touchCanvas();
  for (const code of ["KeyM", "KeyJ", "KeyK"]) await touch.keyboard.down(code);
  for (const code of ["KeyK", "KeyJ", "KeyM"]) await touch.keyboard.up(code);
  await touch.waitForFunction(menu => document.getElementById("prospector").toDataURL() !== menu, linkedMenu);

  await touch.locator(".links summary").click();
  await touch.locator("#left-link").uncheck();
  await touch.locator("#right-link").uncheck();
  await touch.getByRole("button", { name: "Enlarge Prospector display" }).click();
  const enlarged = touch.locator("#enlarged-display");
  const cdp = await touch.context().newCDPSession(touch);
  async function gesture({ dx = 0, dy = 0, hold = 0 } = {}) {
    const box = await enlarged.boundingBox();
    const x = box.x + box.width / 2;
    const y = box.y + box.height / 2;
    await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x, y }] });
    if (dx || dy) {
      await cdp.send("Input.dispatchTouchEvent", {
        type: "touchMove",
        touchPoints: [{ x: x + dx, y: y + dy }],
      });
    }
    if (hold) await touch.waitForTimeout(hold);
    await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
    await touch.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
  }
  const awaitingToucan = await touchCanvas();
  await gesture({ hold: 580 });
  assert.ok((await touchCanvas()) !== awaitingToucan);
  await gesture({ dy: 40 });
  await gesture({ dy: -40 });
  await gesture({ dy: 40 });
  await gesture({ dx: 40 });
  await gesture({ dx: -40 });
  await gesture();
  const standaloneDebug = await touchCanvas();
  await gesture();
  assert.ok((await touchCanvas()) !== standaloneDebug);
  await gesture({ hold: 580 });
  assert.ok((await touchCanvas()) === standaloneDebug);
  await gesture();
  await gesture();
  await gesture({ dy: 40 });
  await gesture();
  assert.equal(await touch.getByText("UF2 bootloader", { exact: true }).filter({ visible: true }).count(), 0);
  await gesture({ dy: 40 });
  await gesture();
  await gesture({ dy: 40 });
  await gesture();
  assert.equal(await touch.getByText("UF2 bootloader", { exact: true }).filter({ visible: true }).count(), 1);
  assert.equal(await touch.locator("#prospector-link").isChecked(), false);
  assert.ok(await touch.locator('#left-link + .bootloader-status').isHidden());
  assert.ok(await touch.locator('#right-link + .bootloader-status').isHidden());
  assert.equal(await touch.locator("#text-output").inputValue(), "");
  await cdp.detach();
  await touch.close();

  for (const ratio of [1.25, 1.5, 2]) {
    const density = await browser.newPage({ viewport: { width: 1024, height: 768 }, deviceScaleFactor: ratio });
    await density.goto(url);
    await density.locator('#runtime[data-status="ready"]').waitFor();
    await checkGeometry(density);
    await checkDeviceScale(density, "#prospector", Math.round(ratio));
    await checkDeviceScale(density, "#toucan-display", Math.round(ratio));
    await density.getByRole("button", { name: "Enlarge Prospector display" }).click();
    await checkDeviceScale(density, "#enlarged-display", 2 * Math.round(ratio));
    await density.close();
  }

  const compositor = await chromium.launch({
    channel: "chrome",
    args: ["--force-device-scale-factor=1.25", "--window-size=1280,1000"],
  });
  try {
    const scaled = await compositor.newPage({ viewport: null });
    await scaled.goto(url);
    await scaled.locator('#runtime[data-status="ready"]').waitFor();
    await scaled.keyboard.press("BracketRight");
    await scaled.keyboard.down("KeyF");
    await scaled.evaluate(async () => {
      window.scrollTo(0, 71);
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    });
    const cdp = await scaled.context().newCDPSession(scaled);
    const { data } = await cdp.send("Page.captureScreenshot", { format: "png", fromSurface: true });
    const mismatches = await scaled.evaluate(async data => {
      const captured = await fetch(`data:image/png;base64,${data}`).then(response => response.blob());
      const image = await createImageBitmap(captured);
      const frame = new OffscreenCanvas(image.width, image.height).getContext("2d");
      frame.drawImage(image, 0, 0);
      return ["prospector", "toucan-display"].map(id => {
        const panel = document.getElementById(id);
        const { left, top } = panel.getBoundingClientRect();
        const { width, height } = panel;
        const inset = id === "prospector" ? 30 : 0;
        const rows = height - 2 * inset;
        const native = panel.getContext("2d").getImageData(0, inset, width, rows).data;
        const visible = frame.getImageData(Math.round(left * devicePixelRatio),
          Math.round(top * devicePixelRatio) + inset, width, rows).data;
        return native.reduce((count, channel, index) => count + Number(channel !== visible[index]), 0);
      });
    }, data);
    assert.deepEqual(mismatches, [0, 0]);
  } finally {
    await compositor.close();
  }

  await page.bringToFront();
  await page.keyboard.press("BracketRight");
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
  const cleanBraille = await canvas("#prospector");
  await page.keyboard.down("KeyF");
  await page.waitForFunction(before => document.getElementById("prospector").toDataURL() !== before, cleanBraille);
  const singleDot = await canvas("#prospector");
  await page.keyboard.down("KeyD");
  await page.waitForFunction(before => document.getElementById("prospector").toDataURL() !== before, singleDot);
  await page.keyboard.up("KeyD");
  await page.waitForFunction(expected => document.getElementById("prospector").toDataURL() === expected, singleDot);
  assert.equal(await output(), "");
  await page.keyboard.up("KeyF");
  assert.equal(await output(), "a");
  await page.evaluate(() => {
    for (const [type, code] of [["keydown", "KeyF"], ["keydown", "KeyD"], ["keyup", "KeyD"], ["keyup", "KeyF"]]) {
      window.dispatchEvent(new KeyboardEvent(type, { code }));
    }
  });
  assert.equal(await output(), "ab");
  await page.keyboard.press("KeyA");
  assert.equal(await output(), "a");

  async function openAlphabet() {
    await chord("Space", "KeyF", "KeyD", "KeyS", "KeyV", "KeyC");
    await chord("Space", "KeyF", "KeyD", "KeyS", "KeyC");
    await chord("Space", "KeyA");
  }

  await openAlphabet();
  const landscapeStart = await canvas("#prospector");
  await chord("Space", "KeyF", "KeyV", "KeyC", "KeyX");
  const landscapePage = await canvas("#prospector");
  assert.notEqual(landscapePage, landscapeStart);
  await chord("KeyF", "KeyD", "KeyS", "KeyV", "KeyC");
  assert.equal(await canvas("#prospector"), landscapePage);

  await page.locator(".links summary").click();
  await page.locator("#prospector-link").uncheck();
  await page.locator("#text-output").focus();
  await chord("Space", "KeyF", "KeyS", "KeyD");
  const portraitStart = await canvas("#toucan-display");
  await chord("Space", "KeyF", "KeyV", "KeyC", "KeyX");
  const portraitPage = await canvas("#toucan-display");
  assert.notEqual(portraitPage, portraitStart);
  await chord("KeyF", "KeyS", "KeyV");
  assert.equal(await canvas("#toucan-display"), portraitPage);

  await page.reload();
  await page.locator('#runtime[data-status="ready"]').waitFor();
  await page.locator(".links summary").click();
  await page.locator("#usb-link").uncheck();
  await page.locator("#text-output").focus();
  async function openBootloaderMenu() {
    await chord("Space", "KeyF", "KeyD", "KeyS", "KeyV", "KeyC");
    await chord("Space", "KeyF", "KeyV", "KeyC");
    await chord("Space", "KeyA");
  }
  const leftLink = page.locator("#left-link");
  const rightLink = page.locator("#right-link");
  const relayLink = page.locator("#prospector-link");
  await page.keyboard.press("BracketRight");
  await openBootloaderMenu();
  await chord("Space", "KeyV");
  await chord("Space", "KeyA");
  await chord("Space", "KeyV");
  await chord("Space", "KeyA");
  assert.ok(await rightLink.isChecked() && await relayLink.isChecked());
  await chord("Space", "KeyV");
  await chord("Space", "KeyA");
  assert.ok(await leftLink.isChecked());
  assert.ok(!await rightLink.isChecked() && !await relayLink.isChecked());
  assert.equal(await page.getByText("UF2 bootloader", { exact: true }).filter({ visible: true }).count(), 2);
  await relayLink.check();
  await page.locator("#usb-link").check();
  await page.locator("#text-output").focus();
  await page.keyboard.press("BracketRight");
  await page.keyboard.press("KeyQ");
  await page.keyboard.press("KeyP");
  assert.equal(await output(), "q");

  await rightLink.check();
  await page.locator("#text-output").focus();
  await page.keyboard.press("BracketRight");
  await openBootloaderMenu();
  await chord("Space", "KeyA");
  await chord("Space", "KeyV", "KeyC", "KeyX");
  await chord("Space", "KeyA");
  assert.ok(!await leftLink.isChecked());
  assert.ok(await rightLink.isChecked() && await relayLink.isChecked());
  await leftLink.check();
  assert.equal(await page.locator("#mode").textContent(), "QWERTY");
  assert.equal(await page.getByText("UF2 bootloader", { exact: true }).filter({ visible: true }).count(), 0);

  await page.reload();
  await page.locator('#runtime[data-status="ready"]').waitFor();
  await page.locator(".links summary").click();
  for (const id of ["right-link", "prospector-link", "usb-link", "toucan-usb-link", "bluetooth-link"]) {
    await page.locator("#" + id).uncheck();
  }
  assert.ok(await leftLink.isChecked());
  await page.locator("#text-output").focus();
  await page.keyboard.press("BracketRight");
  await chord("MetaLeft", "KeyF", "KeyD", "KeyS", "KeyV", "KeyC");
  await chord("KeyF", "KeyD", "KeyS", "KeyC");
  const onlyLeftCategories = await canvas("#toucan-display");
  await chord("MetaLeft", "KeyA");
  await page.waitForFunction(categories => document.getElementById("toucan-display").toDataURL() !== categories, onlyLeftCategories);
  const onlyLeftAlphabet = await canvas("#toucan-display");
  await chord("MetaLeft", "KeyF", "KeyV", "KeyC", "KeyX");
  await page.waitForFunction(alphabet => document.getElementById("toucan-display").toDataURL() !== alphabet, onlyLeftAlphabet);
  await chord("MetaLeft", "KeyF", "KeyD");
  await chord("MetaLeft", "KeyF", "KeyD");
  await chord("KeyF", "KeyV", "KeyC");
  await chord("MetaLeft", "KeyA");
  await chord("MetaLeft", "KeyA");
  for (let row = 0; row < 3; row += 1) await chord("MetaLeft", "KeyX");
  assert.equal(await page.locator("#last-report").textContent(), "00 00 00 00 00 00 00 00");
  await chord("MetaLeft", "KeyA");
  assert.equal(await output(), "");
  assert.ok(!await leftLink.isChecked() && !await rightLink.isChecked() && !await relayLink.isChecked());
  assert.ok(await page.locator('#left-link + .bootloader-status').isVisible());
  assert.ok(await page.locator('#right-link + .bootloader-status').isHidden());
  assert.ok(await page.locator('#prospector-link + .bootloader-status').isHidden());

  assert.deepEqual(errors, []);
  console.log("Chrome verified QWERTY, Braille release timing, Reference paging, all seven host targets, Prospector touch and standalone UF2, persistence, and native and enlarged display grids.");
} finally {
  await browser?.close();
  server.kill();
}
