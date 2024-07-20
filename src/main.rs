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
use platform::{Brightness, Platform};
use rand::Rng;

mod platform;

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
    let mut platform = platform::new_esp32();
    log::info!("Hello, world!");

    let mut rng = rand::thread_rng();
    log::info!("allocating buffers");
    let mut buffer: Box<[Rgb565; <platform as Platform>::LCD_PIXEL_COUNT]> = {
        let mut buf = Box::<[Rgb565]>::new_uninit_slice(<platform as Platform>::LCD_PIXEL_COUNT);
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
    const FRAMES_PER_SHADE: usize = 10;
    let total_frames: usize = FRAMES_PER_SHADE * shades_of_red.len();

    loop {
        for (idx, &bgcolor) in shades_of_red.iter().enumerate() {
            let intensity = idx as i32 / (shades_of_red.len() as i32 / MAX_INTENSITY);

            for frame in 0..FRAMES_PER_SHADE {
                let brightness = Brightness({
                    let curr_frame = idx * FRAMES_PER_SHADE + frame;
                    let linear: f32 = curr_frame as f32 / total_frames as f32;
                    linear.powf(3.0)
                });
                platform.led0().set_brightness(brightness).unwrap();
                platform.led1().set_brightness(brightness).unwrap();

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

                platform.lcd().fill_contiguous(
                    &Rectangle::new(Point::zero(), lcd.size()),
                    buffer.iter().copied(),
                )
                .unwrap();

                platform.sleep(Duration::from_millis(10));
            }
        }
    }
}
