# Shared nRF52840 platform

`nrf52840/` is the firmware Cargo workspace. Its manifest selects binary entry points from [toucan/](../toucan/), [paintbrush/](../paintbrush/), only. No standalone, example, diagnostic, or reset firmware entry points remain. Shared code provides clocks, USB HID, Nordic radio initialization and authentication, persistent bonds/settings, battery sensing, and the Prospector display and touch buses.

Use the root target catalog. Firmware builds require WABT 1.0.42 for ahead-of-time compilation of the production component.

```sh
cargo xtask targets
cargo xtask build toucan-prospector
cargo xtask build paintbrush-prospector
```

`cargo xtask build` compiles the selected Wavelet source, tests its typing and display interfaces with Wasmi, cross-builds firmware, and writes ELF, component, and UF2 outputs under `dist/`. `./scripts/build` regenerates the Wavelet components and builds exactly six images into `dist/release/`: left half, right half, and Prospector for each keyboard. It validates the target flash and native stack limits before replacing the UF2s and their SHA-256 checksums, preventing stale variants from leaking into a release. All current targets use `thumbv7em-none-eabihf`.

## Prospector display

The Waveshare 1.69-inch Touch LCD Module uses an ST7789V2, with 240×280 native geometry and a 20-row controller offset. Both kits use the display in 280×240 landscape, with a 20-column controller offset in this orientation. Touch requires the CST816 controller and four additional wires.

| Signal | XIAO pin |
| --- | --- |
| SCK | P1.13 |
| MOSI | P1.15 |
| CS, active low | P1.14 / D9 |
| DC, low for commands | P1.12 / D7 |
| Reset, active low | P0.29 / D3 |
| Backlight, active high | P1.11 / D6 |

For boards wired for touch, connect the following signals in addition to the LCD wiring. Existing boards with only the LCD wired use the same receiver firmware and keyboard controls. [Waveshare module pinout](https://www.waveshare.com/wiki/1.69inch_Touch_LCD_Module)

| Touch signal | XIAO pin |
| --- | --- |
| TP_IRQ, active low | P0.02 / D0 |
| TP_RST, active low | P0.03 / D1 |
| TP_SDA | P0.04 / D4 |
| TP_SCL | P0.05 / D5 |

Tap to open the menu or selected item, swipe to navigate, and hold to go back. With Prospector on a USB data cable, touch can open **Main Menu → Debug → UF2 Bootloader** and select the local receiver while the keyboard is disconnected. The Toucan keyboard and its linked Prospector share the active menu. The current touch wiring and gestures still need hardware validation.

Normal firmware uses SPI2 at 4 MHz. Hardware testing found a blank LCD with the former SPI3/16 MHz configuration; slow software SPI and then SPI2 displayed content successfully. Initialization explicitly enables extended register writes and configures the panel before leaving sleep. Backlight GPIO control is active-high, despite the upstream overlay's initial PWM inversion setting.

Both kits draw Wavelet scenes through LVGL. The panel driver sends changed RGB565 regions and caps redraws at 30 updates/s. A full 280×240 frame at 4 MHz needs at least 269 ms of transfer time. Physical update rates for the current firmware still need measurement.

[Upstream Prospector XIAO overlay](https://github.com/carrefinho/prospector-zmk-module/blob/main/boards/shields/prospector_adapter/boards/seeeduino_xiao_ble.overlay), [Waveshare panel](https://www.waveshare.com/product/displays/lcd-oled/lcd-oled-3/1.69inch-touch-lcd-module.htm), and [Waveshare Arduino demo](https://files.waveshare.com/wiki/1.69inch-Touch-LCD-Module/1.69inch_Touch_LCD_Arduino.zip).

The temporary software-SPI diagnostic and standalone pairing-reset images have been removed. Their successful display fixes remain in the shared driver. Pairing management remains part of the keyboard applications.
