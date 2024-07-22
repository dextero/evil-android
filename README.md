# Setup

# Hardware pinout

## LCD

| ST7735 | ESP32 GPIO | description                |
|--------|------------|----------------------------|
| LED    | GPIO 18    | backlight                  |
| SCK    | GPIO 14    | SPI SCL                    |
| SDA    | GPIO 13    | SPI MOSI                   |
| A0     | GPIO 17    | ? command/data selection ? |
| RESET  | GPIO 16    | LCD reset, active low      |
| CS     | GPIO 15    | chip select                |

## LEDs

| ESP32 GPIO | description      |
|------------|------------------|
| GPIO 19    | left eye / LED0  |
| GPIO 21    | right eye / LED1 |

## Linux simulator build

Make sure to use the `main` branch. `cargo run` will do the trick.

## ESP32 build

See "Rust on ESP" book for instructions for setting up the
toolchain: https://docs.esp-rs.org/book/introduction.html

Either use the `esp32` branch, or:

1. Uncomment the line in `build.rs` file.

   `#[cfg]` are not defined when building it, and `embuild::espidf` just
   doesn't exist on non-ESP builds so a runtime check fails to compile. Of
   course there's a 6-year-old issue about this.
   https://github.com/rust-lang/cargo/issues/4932

2. Uncomment the `channel = "esp"` line in `rust-toolchain.toml` file, and
   comment the `channel = "stable"` one.

3. Uncomment the `#target = "xtensa-esp32-espidf"` line in `.cargo/config.toml`.
