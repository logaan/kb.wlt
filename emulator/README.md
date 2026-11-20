# Toucan browser emulator

The browser runs the checked-in Toucan keyboard, braille and display guest as one WebAssembly Component instance per board that runs a guest. The left half processes input and draws its display, while the Prospector renders the forwarded display model on its LCD. Jco generates the browser bindings from the same component for both. The Rust host reuses the Toucan matrix layer switch, braille session, USB report handling, and LVGL renderer.

## Build and run

Use Node.js 22.12 or later, Rust with the `wasm32-unknown-unknown` target, and a Wasm-capable Clang such as the one in the WASI SDK.

```sh
rustup target add wasm32-unknown-unknown
npm --prefix emulator ci
CC_wasm32_unknown_unknown=/path/to/wasi-sdk/bin/clang npm --prefix emulator run build
npm --prefix emulator start
```

Open [localhost:4173](http://127.0.0.1:4173). After changing Wavelet code, run `./scripts/build-components.sh` from the repo root before rebuilding the emulator. The build writes the static site to `emulator/dist`. Run `npm --prefix emulator test` for unit checks and `npm --prefix emulator run test:browser` after a build for the full smoke test with Google Chrome installed.

## Keyboard and displays

Click keycaps, tap them with multiple fingers, or use a physical keyboard. The physical `[` key maps to Escape, `/` maps to the bottom-right minus, and `\` maps to the right thumb Shift. Tap `]` to switch between QWERTY and braille. Hold it down to use braille until release. In braille, the keycaps show their dot or Space assignment. Hold the keys in a chord together, then release them. Keys released less than 50 milliseconds apart can join the same chord. Chord A alone for Backspace.

Chord Space with F, D, S, J and K to open the braille menu. Repeat that chord for Main Menu. From either screen, chord Space with F, D, S and K for Reference, then Space with A to open the selected category.

The left Toucan half contains the monochrome display. The Prospector shows the color display above both halves. Select either display to expand a live preview inline while continuing to type. The Prospector clips its corners to the equivalent of a 30-pixel radius at native resolution. The MIP stays rectangular.

To switch between **Prospector USB** and **Toucan Bluetooth**, press and hold the outer left Braille thumb (`]`) first. Add Backspace and left Meta, then hold all three for two seconds. The active route appears below the keyboard. Both routes use the left half as the application host. Expand **Simulated connections** to change the battery levels or link states. Browser storage persists transport, menu and binding settings.

The simulator validates components, key input, and display output. BLE, battery sensing, USB transport, and the physical displays require hardware validation.

The keys follow Toucan's column stagger and thumb splay. Both displays scale with the keyboard's 18 × 17 mm switch pitch. The Prospector uses the Waveshare panel's 32.634 × 27.972 mm landscape active area and the Toucan display uses Sharp's 20.88 × 24.36 mm active area. [Toucan layout](https://github.com/beekeeb/zmk-keyboard-toucan/blob/main/boards/shields/toucan/toucan.dtsi) · [Toucan spacing](https://beekeeb.com/toucan-keyboard/) · [Prospector LCD](https://www.waveshare.com/wiki/1.69inch_Touch_LCD_Module) · [Sharp MIP](https://global.sharp/products/device/lineup/selection/lcd/mobile/index.html)
