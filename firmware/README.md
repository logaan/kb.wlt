# Shared nRF52840 platform

`nrf52840/` is the firmware Cargo workspace. Its manifest selects binary entry points from [toucan/](../toucan/), [paintbrush/](../paintbrush/), only. No standalone, example, diagnostic, or reset firmware entry points remain. Shared code provides clocks, USB HID, Nordic radio initialization and authentication, persistent bonds/settings, battery sensing, and the Prospector LCD bus.

Use the root target catalog. Firmware builds require WABT 1.0.42 for ahead-of-time compilation of the production component.

```sh
cargo xtask targets
cargo xtask build toucan-prospector
cargo xtask build prospector-pair
```

`cargo xtask build` compiles the selected Wavelet source, tests its typing and display interfaces with Wasmi, cross-builds firmware, and writes ELF, component, and UF2 outputs under `dist/`. `./scripts/build-artifacts.sh` builds exactly six images from checked-in components into `dist/release/`: left half, right half, and Prospector for each keyboard. It validates the target flash and native stack limits before replacing the UF2s and their SHA-256 checksums, preventing stale variants from leaking into a release. All current targets use `thumbv7em-none-eabihf`.

## Prospector display

The Waveshare 1.69-inch Touch LCD Module uses an ST7789V2, with 240×280 native geometry and a 20-row controller offset. Both kits use the display in 280×240 landscape, with a 20-column controller offset in this orientation. Touch and automatic brightness are not implemented.

| Signal | XIAO pin |
| --- | --- |
| SCK | P1.13 |
| MOSI | P1.15 |
| CS, active low | P1.14 / D9 |
| DC, low for commands | P1.12 / D7 |
| Reset, active low | P0.29 / D3 |
| Backlight, active high | P1.11 / D6 |

Normal firmware uses SPI2 at 4 MHz. Hardware testing found a blank LCD with the former SPI3/16 MHz configuration; slow software SPI and then SPI2 displayed content successfully. Initialization explicitly enables extended register writes and configures the panel before leaving sleep. Backlight GPIO control is active-high, despite the upstream overlay's initial PWM inversion setting.

Both kits draw Wavelet scenes through LVGL. The panel driver sends changed RGB565 regions and caps redraws at 30 updates/s. A full 280×240 frame at 4 MHz needs at least 269 ms of transfer time. Physical update rates for the current firmware still need measurement.

[Upstream Prospector XIAO overlay](https://github.com/carrefinho/prospector-zmk-module/blob/main/boards/shields/prospector_adapter/boards/seeeduino_xiao_ble.overlay), [Waveshare panel](https://www.waveshare.com/product/displays/lcd-oled/lcd-oled-3/1.69inch-touch-lcd-module.htm), and [Waveshare Arduino demo](https://files.waveshare.com/wiki/1.69inch-Touch-LCD-Module/1.69inch_Touch_LCD_Arduino.zip).

The temporary software-SPI diagnostic and standalone pairing-reset images have been removed. Their successful display fixes remain in the shared driver. Pairing management remains part of the keyboard applications.
