#![feature(new_uninit)]

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{Dimensions, OriginDimensions, Point, Size},
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::{PixelColor, Rgb565},
    prelude::RgbColor,
    primitives::{PointsIter, Rectangle},
    text::{Alignment, Text},
    Drawable, Pixel,
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
use itertools::izip;
use rand::Rng;
use st7735_lcd::ST7735;

fn intensify(rng: &mut impl Rng, point: Point, amplitude: i32) -> Point {
    Point::new(
        point.x + rng.gen_range(-amplitude..amplitude),
        point.y + rng.gen_range(-amplitude..amplitude),
    )
}

fn fb_diff<C: PixelColor>(size: Size, old: impl Iterator<Item = C>, new: impl IntoIterator<Item = C>) -> impl IntoIterator<Item = Pixel<C>> {
    let area = Rectangle::new(Point::zero(), size);
    izip!(area.points(), old, new).filter_map(|(coords, o, n)| {
        if o == n {
            None
        } else {
            Some(Pixel(coords, n.clone()))
        }
    })
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
    lcd.clear(Rgb565::BLACK).unwrap();
    lcd_led.set_high().unwrap();

    let mut rng = rand::thread_rng();
    FreeRtos::delay_ms(100);
    log::info!("allocating buffers");
    let mut front_buffer: Box<[Rgb565; LCD_PIXEL_COUNT]> = {
        let mut buf = Box::<[Rgb565]>::new_uninit_slice(LCD_PIXEL_COUNT);
        unsafe {
            buf.iter_mut().for_each(|col| { col.write(Rgb565::BLACK); });
            buf.assume_init().try_into().unwrap()
        }
    };
    let mut back_buffer = front_buffer.clone();
    FreeRtos::delay_ms(100);
    log::info!("entering draw loop");
    loop {
        let mut framebuffer = FrameBuf::new(
            TryInto::<&mut [Rgb565; LCD_PIXEL_COUNT]>::try_into(back_buffer.as_mut_slice()).unwrap(),
            LCD_SIZE.width.try_into().unwrap(),
            LCD_SIZE.height.try_into().unwrap(),
        );

        framebuffer.clear(Rgb565::BLACK).unwrap();
        Text::with_alignment(
            "Hello LCD!",
            intensify(&mut rng, lcd.bounding_box().center(), 10),
            MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE),
            Alignment::Center,
        )
        .draw(&mut framebuffer)
        .unwrap();

        let diff = fb_diff(lcd.size(), front_buffer.iter().copied(), back_buffer.iter().copied());
        lcd.draw_iter(diff).unwrap();
        std::mem::swap(&mut front_buffer, &mut back_buffer);

        FreeRtos::delay_ms(10);
    }
}
