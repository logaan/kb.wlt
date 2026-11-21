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

Open [localhost:4173](http://127.0.0.1:4173). After changing Wavelet code, run `./scripts/build --wasm-only` from the repo root before rebuilding the emulator. The build writes the static site to `emulator/dist`. Run `npm --prefix emulator test` for unit checks and `npm --prefix emulator run test:browser` after a build for the full smoke test with Google Chrome installed.

## Keyboard and displays

Click keycaps, tap them with multiple fingers, or use a physical keyboard. The physical `[` key maps to Escape and `/` maps to the bottom-right minus. Tap either outer thumb (`]` or `\`) to toggle Braille, or hold it for momentary Braille. Both halves have a full eight-dot cell layout. The inner thumbs (left Meta and Space) act as Space in Braille. Hold the keys in a chord together, then release them. Keys released less than 50 milliseconds apart can join the same chord. Chord ⡀ alone for Backspace.

Chord ⠟̲ to open the Braille menu. An underlined cell includes Space. Repeat that chord for Main Menu. From either screen, chord **r** for Reference, then ⡀̲ to open the selected category.

The left Toucan half contains the monochrome display. The Prospector shows the color display above both halves. Use **Enlarge** on the Prospector or select the MIP to expand a live preview inline while continuing to type. The Prospector clips its corners to the equivalent of a 30-pixel radius at native resolution. The MIP stays rectangular.

Touch or click either view of the Prospector display to open its menu. Swipe or drag in a direction to move, tap to select, and hold then release to go back. With the keyboard disconnected, the display still opens the menu and can enter its own UF2 bootloader from Debug.

To switch the host target, open the Braille menu and chord **t** for Target. Chord ⠄̲ or ⠠̲ to move and ⡀̲ to choose Prospector, BT 1–5, or USB. The selected target appears below the keyboard. Expand **Simulated connections** to change the battery levels or link states. Browser storage persists the target, menu and binding settings.

In the Braille menu, chord **d** for Debug, then ⡀̲ for **UF2 Bootloader**. Chord ⠠̲ to move down, ⠄̲ to move up, and ⡀̲ to toggle a device. The MIP shows an empty box for an available device, a filled box for a selected device, and an X for an unavailable device. Select **Enter UF2** to request bootloader mode for the selected devices. Turn the devices back on in **Simulated connections** to resume using them.

The simulator validates components, key input, and display output. BLE, battery sensing, USB transport, and the physical displays require hardware validation.

The keys follow Toucan's column stagger and thumb splay. The 280 × 240 Prospector panel and 144 × 168 Toucan MIP scale by whole device pixels to keep text and Braille cells sharp. The keyboard adjusts around the panels, and the MIP sits above the left keys in the same case on narrow screens. [Toucan layout](https://github.com/beekeeb/zmk-keyboard-toucan/blob/main/boards/shields/toucan/toucan.dtsi) · [Toucan spacing](https://beekeeb.com/toucan-keyboard/) · [Prospector LCD](https://www.waveshare.com/wiki/1.69inch_Touch_LCD_Module) · [Sharp MIP](https://global.sharp/products/device/lineup/selection/lcd/mobile/index.html)
