# Shared nRF52840 platform

`nrf52840/` is the firmware Cargo workspace. Its manifest selects binary entry points from [toucan/](../toucan/), [paintbrush/](../paintbrush/), and local platform examples/diagnostics. Shared code provides clocks, USB HID, Nordic radio initialization and authentication, persistent bonds/settings, the component heap, and the Prospector LCD bus.

Use the root target catalog rather than assembling feature combinations by hand:

```sh
cargo xtask targets
cargo xtask build toucan-prospector
cargo xtask build prospector-single
cargo xtask build xiao-led
cargo xtask build prospector-display-test
```

`cargo xtask build` compiles the selected Wavelet source, runs its resource probe, cross-builds firmware, and writes ELF, component, and UF2 outputs under `dist/`. `./scripts/build-artifacts.sh` builds the complete release catalog from checked-in components into `dist/release/`, including the separate resetter firmware. All current targets use `thumbv7em-none-eabihf`.

## Prospector display

The Waveshare 1.69-inch Touch LCD Module uses an ST7789V2, with 240×280 native geometry and a 20-row controller offset. Toucan status is rotated to 280×240 landscape; Paintbrush menus retain portrait geometry. Touch and automatic brightness are not implemented.

| Signal | XIAO pin |
| --- | --- |
| SCK | P1.13 |
| MOSI | P1.15 |
| CS, active low | P1.14 / D9 |
| DC, low for commands | P1.12 / D7 |
| Reset, active low | P0.29 / D3 |
| Backlight, active high | P1.11 / D6 |

Normal firmware uses SPI2 at 4 MHz. Hardware testing found a blank LCD with the former SPI3/16 MHz configuration; slow software SPI and then SPI2 displayed content successfully. Initialization explicitly enables extended register writes and configures the panel before leaving sleep. Backlight GPIO control is active-high, despite the upstream overlay's initial PWM inversion setting.

References: [upstream Prospector XIAO overlay](https://github.com/carrefinho/prospector-zmk-module/blob/main/boards/shields/prospector_adapter/boards/seeeduino_xiao_ble.overlay), [Waveshare panel](https://www.waveshare.com/product/displays/lcd-oled/lcd-oled-3/1.69inch-touch-lcd-module.htm), and [Waveshare Arduino demo](https://files.waveshare.com/wiki/1.69inch-Touch-LCD-Module/1.69inch_Touch_LCD_Arduino.zip).

`prospector-display-test` cycles solid colors with slow software SPI and no keyboard reception. It does not erase saved bonds. Reflash the normal receiver image afterward. `settings-reset-xiao` and `settings-reset-nano`, by contrast, intentionally clear saved pairing records; do not use them for ordinary display diagnosis.
