# Setup

# Pinout

| ST7735 | ESP32 GPIO | description                |
|--------|------------|----------------------------|
| LED    | GPIO 18    | backlight                  |
| SCK    | GPIO 14    | SPI SCL                    |
| SDA    | GPIO 13    | SPI MOSI                   |
| A0     | GPIO 17    | ? command/data selection ? |
| RESET  | GPIO 16    | LCD reset, active low      |
| CS     | GPIO 15    | chip select                |
