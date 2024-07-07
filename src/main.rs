#![feature(new_uninit)]

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{Dimensions, OriginDimensions, Point, Size},
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::RgbColor,
    primitives::Rectangle,
    text::{Alignment, Text},
    Drawable,
};
use embedded_graphics_framebuf::FrameBuf;
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

fn intensify(rng: &mut impl Rng, point: Point, amplitude: i32) -> Point {
    if amplitude == 0 {
        point
    } else {
        Point::new(
            point.x + rng.gen_range(-amplitude..amplitude),
            point.y + rng.gen_range(-amplitude..amplitude),
        )
    }
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
    const LCD_SIZE: Size = Size::new(160, 128);
    const LCD_PIXEL_COUNT: usize = (LCD_SIZE.width * LCD_SIZE.height) as usize;
    let mut lcd = ST7735::new(
        lcd_spi,
        lcd_a0,
        lcd_reset,
        true,
        false,
        LCD_SIZE.width.try_into().unwrap(),
        LCD_SIZE.height.try_into().unwrap(),
    );

    log::info!("initializing LCD");
    lcd.init(&mut FreeRtos).unwrap();
    lcd.set_orientation(&st7735_lcd::Orientation::Landscape)
        .unwrap();
    lcd_led.set_high().unwrap();

    let mut rng = rand::thread_rng();
    log::info!("allocating buffers");
    let mut buffer: Box<[Rgb565; LCD_PIXEL_COUNT]> = {
        let mut buf = Box::<[Rgb565]>::new_uninit_slice(LCD_PIXEL_COUNT);
        unsafe {
            buf.iter_mut().for_each(|col| {
                col.write(Rgb565::BLACK);
            });
            buf.assume_init().try_into().unwrap()
        }
    };
    log::info!("entering draw loop");

    let shades_of_red: [Rgb565; 32] = (0..32)
        .map(|v| Rgb565::new(v, 0, 0))
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    const MAX_INTENSITY: i32 = 3;

    loop {
        for (idx, &bgcolor) in shades_of_red.iter().enumerate() {
            let intensity = idx as i32 / (shades_of_red.len() as i32 / MAX_INTENSITY);

            for frame in 0..10 {
                let mut framebuffer = FrameBuf::new(
                    TryInto::<&mut [Rgb565; LCD_PIXEL_COUNT]>::try_into(buffer.as_mut_slice())
                        .unwrap(),
                    LCD_SIZE.width.try_into().unwrap(),
                    LCD_SIZE.height.try_into().unwrap(),
                );

                framebuffer.clear(bgcolor).unwrap();
                Text::with_alignment(
                    "Analyzing Android.bp...",
                    intensify(&mut rng, lcd.bounding_box().center(), intensity),
                    MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE),
                    Alignment::Center,
                )
                .draw(&mut framebuffer)
                .unwrap();

                lcd.fill_contiguous(&Rectangle::new(Point::zero(), lcd.size()), buffer.iter().copied()).unwrap();

                FreeRtos::delay_ms(10);
            }
        }
    }
}
