import { execFileSync } from "node:child_process";
import { copyFile, mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { transpile } from "@bytecodealliance/jco-transpile";

const emulator = fileURLToPath(new URL(".", import.meta.url));
const repo = resolve(emulator, "..");
const dist = resolve(emulator, "dist");
const components = resolve(dist, "components");
const target = resolve(repo, "target", "emulator");
const release = resolve(target, "wasm32-unknown-unknown", "release");
const guest = resolve(repo, "clients/components/generated/toucan-keymap/keyboard.component.wasm");

function run(command, args, env = process.env) {
  execFileSync(command, args, { cwd: repo, env, stdio: "inherit" });
}

await mkdir(components, { recursive: true });
const { files, exports } = await transpile(guest, {
  name: "toucan",
  instantiation: "async",
  base64Cutoff: 0,
  nodejsCompat: false,
  emitTypescriptDeclarations: false,
});
if (!["keyboard", "braille", "display"].every(name => exports.some(([key]) => key === name))) {
  throw new Error("Toucan component is missing an interface. Run ./scripts/build-components.sh.");
}

const env = { ...process.env, CARGO_TARGET_DIR: target };
run("cargo", [
  "build", "--locked", "--release", "--target", "wasm32-unknown-unknown", "-p", "kb-toucan-emulator",
], env);

await Promise.all([
  ...Object.entries(files).map(([file, data]) => writeFile(resolve(components, file), data)),
  copyFile(resolve(release, "kb_toucan_emulator.wasm"), resolve(components, "toucan-adapter.wasm")),
  ...["index.html", "style.css", "main.js", "keys.js", "display.js"].map(file =>
    copyFile(resolve(emulator, file), resolve(dist, file)),
  ),
]);
console.log(`Built Toucan emulator at ${dist}`);
