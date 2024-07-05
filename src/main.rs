use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::Point,
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::RgbColor,
    text::{Alignment, Text},
    Drawable,
};
use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{AnyInputPin, OutputPin, PinDriver, Pins},
    peripherals::Peripherals,
    spi::{
        config::{Config, MODE_3},
        SpiDeviceDriver, SpiDriverConfig,
    },
    units::FromValueType,
};
use rand::Rng;
use st7735_lcd::ST7735;

fn intensify(rng: &mut impl Rng, mut point: Point, amplitude: i32) -> Point {
    Point::new(
        point.x + rng.gen_range(-amplitude..amplitude),
        point.y + rng.gen_range(-amplitude..amplitude),
    )
}

fn main() {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();
    log::info!("Hello, world!");

    let Peripherals {
        spi2: lcd_spi,
        pins:
            Pins {
                gpio13: lcd_spi_mosi,
                gpio14: lcd_spi_scl,
                gpio15: lcd_spi_cs,
                gpio16: lcd_reset,
                gpio17: lcd_a0,
                gpio18: lcd_led,
                ..
            },
        ..
    } = Peripherals::take().unwrap();
    let lcd_spi = SpiDeviceDriver::new_single(
        lcd_spi,
        lcd_spi_scl,
        lcd_spi_mosi,
        <Option<AnyInputPin>>::None,
        Some(lcd_spi_cs),
        &SpiDriverConfig::new(),
        &Config::new().baudrate(26.MHz().into()).data_mode(MODE_3),
    )
    .unwrap();
    let lcd_reset = PinDriver::output(lcd_reset.downgrade_output()).unwrap();
    let lcd_a0 = PinDriver::output(lcd_a0.downgrade_output()).unwrap();
    let mut lcd_led = PinDriver::output(lcd_led.downgrade_output()).unwrap();
    const LCD_SIZE: (u32, u32) = (160, 128);
    let mut lcd = ST7735::new(
        lcd_spi, lcd_a0, lcd_reset, true, false, LCD_SIZE.0, LCD_SIZE.1,
    );
    lcd.init(&mut FreeRtos).unwrap();
    lcd.set_orientation(&st7735_lcd::Orientation::Landscape)
        .unwrap();
    lcd_led.set_high().unwrap();

    let mut rng = rand::thread_rng();

    loop {
        lcd.clear(Rgb565::BLACK).unwrap();
        Text::with_alignment(
            "Hello LCD!",
            intensify(
                &mut rng,
                Point::new(
                    (LCD_SIZE.0 / 2).try_into().unwrap(),
                    (LCD_SIZE.1 / 2).try_into().unwrap(),
                ),
                10,
            ),
            MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE),
            Alignment::Center,
        )
        .draw(&mut lcd)
        .unwrap();

        FreeRtos::delay_ms(10);
    }
}
