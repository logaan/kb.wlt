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
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
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
  await checkCorners("#nice-view", false);
  const initialProspector = await canvas("#prospector");
  const initialNiceView = await canvas("#nice-view");
  assert.notEqual(initialProspector, initialNiceView);

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
  assert.equal(await page.locator(".dot-cell").count(), 8);
  assert.equal(await page.getByRole("button", { name: "⠁ (F)" }).count(), 1);
  await page.keyboard.down("KeyF");
  await page.waitForTimeout(50);
  assert.ok((await canvas("#prospector")) !== initialProspector, (await page.locator("#runtime").textContent()) + " " + errors.join(" / "));
  assert.notEqual(await canvas("#nice-view"), initialNiceView);
  await page.keyboard.up("KeyF");
  assert.equal(await output(), "qW e-\t\npa");

  async function chord(...codes) {
    for (const code of codes) await page.keyboard.down(code);
    for (const code of codes.toReversed()) await page.keyboard.up(code);
    await page.waitForTimeout(40);
  }
  await chord("Space", "KeyF", "KeyD", "KeyS", "KeyJ", "KeyK");
  const namespace = await canvas("#prospector");
  await chord("Space", "KeyF", "KeyD", "KeyS", "KeyK");
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
  await page.getByRole("button", { name: "Enlarge left Toucan nice!view display" }).click();
  await checkCorners("#enlarged-display", false);
  await page.getByRole("button", { name: "Close", exact: true }).click();
  assert.equal(await output(), "qW e-\t\npa");

  await page.keyboard.press("BracketRight");
  assert.equal(await page.locator("#mode").textContent(), "QWERTY");
  await page.keyboard.down("BracketRight");
  await page.waitForTimeout(240);
  assert.equal(await page.locator("#mode").textContent(), "Braille");
  await chord("KeyF");
  await page.keyboard.up("BracketRight");
  assert.equal(await page.locator("#mode").textContent(), "QWERTY");

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

  await page.locator("#usb-link").uncheck();
  await page.locator("#text-output").focus();
  const beforeSwitch = await output();
  await page.keyboard.press("KeyP");
  assert.equal(await output(), beforeSwitch);
  for (const code of ["BracketRight", "Backspace", "MetaLeft"]) await page.keyboard.down(code);
  await page.waitForTimeout(2100);
  assert.equal(await page.locator("#transport-bluetooth").isChecked(), true);
  for (const code of ["MetaLeft", "Backspace", "BracketRight"]) await page.keyboard.up(code);
  assert.equal(await page.locator("#output-heading").textContent(), "Bluetooth output");
  assert.equal(await output(), beforeSwitch);

  const standaloneProspector = await canvas("#prospector");
  await page.keyboard.press("KeyP");
  assert.equal(await output(), beforeSwitch + "p");
  await page.keyboard.press("BracketRight");
  const standaloneHome = await canvas("#nice-view");
  await page.keyboard.down("KeyF");
  await page.waitForTimeout(50);
  assert.notEqual(await canvas("#nice-view"), standaloneHome);
  await page.keyboard.up("KeyF");
  assert.equal(await output(), beforeSwitch + "pa");
  assert.equal(await canvas("#prospector"), standaloneProspector);
  await page.reload();
  await page.locator('#runtime[data-status="ready"]').waitFor();
  assert.equal(await page.locator("#transport-bluetooth").isChecked(), true);

  await page.keyboard.press("BracketRight");
  await chord("Space", "KeyF", "KeyD", "KeyS", "KeyJ", "KeyK");
  await chord("Space", "KeyF", "KeyD", "KeyS", "KeyJ", "KeyK");
  await chord("Space", "KeyJ");
  await chord("Space", "KeyJ");
  await chord("Space", "KeyA");
  await chord("Space", "KeyA");
  await chord("Space", "KeyA");
  await chord("KeyF");
  await chord("KeyF", "KeyD");
  await chord("KeyF", "KeyJ");
  await chord("KeyA");
  await chord("Space", "KeyA");
  assert.ok(await page.evaluate(() => localStorage.getItem("kb.wlt.toucan.settings")));
  assert.equal(await output(), "");
  await page.keyboard.press("BracketRight");
  await page.keyboard.press("BracketRight");
  await chord("KeyF");
  assert.equal(await output(), "b");

  async function checkScale() {
    const scale = await page.evaluate(() => {
      const rect = id => document.querySelector(id).getBoundingClientRect();
      const [tab, q] = [...document.querySelectorAll("#left-keys .key")];
      const ctrl = document.querySelectorAll("#left-keys .key")[6];
      const dx = q.getBoundingClientRect().left - tab.getBoundingClientRect().left;
      const dy = ctrl.getBoundingClientRect().top - tab.getBoundingClientRect().top;
      const prospector = rect("#prospector");
      const niceView = rect("#nice-view");
      return [prospector.width * 18 / dx, prospector.height * 17 / dy,
        niceView.width * 18 / dx, niceView.height * 17 / dy, dx / dy];
    });
    for (const [index, expected] of [32.634, 27.972, 20.88, 24.36, 18 / 17].entries()) {
      assert.ok(Math.abs(scale[index] - expected) < expected * 0.01, `Scale mismatch ${scale[index]} vs ${expected}`);
    }
  }
  await checkScale();
  await page.setViewportSize({ width: 390, height: 844 });
  const overflow = await page.evaluate(() =>
    document.documentElement.scrollWidth > document.documentElement.clientWidth);
  assert.equal(overflow, false);
  await checkScale();
  const [workbench, left, right] = await Promise.all([".workbench", ".board-left", ".board-right"].map(selector => page.locator(selector).boundingBox()));
  assert.ok(left.x >= workbench.x && right.x + right.width <= workbench.x + workbench.width);
  assert.deepEqual(errors, []);
  console.log("Chrome verified QWERTY, punctuation, modifiers, pointer keycaps, Braille tap and hold, menu, binding editing, inline reference, simulated USB and Bluetooth routes, and display scale at desktop and mobile widths.");
} finally {
  await browser?.close();
  server.kill();
}
