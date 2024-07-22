use std::time::Duration;

use anyhow::{Context, Result};
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{Dimensions, Point, Size},
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::{PixelColor, Rgb565},
    prelude::RgbColor,
    text::{Alignment, Text},
    Drawable,
};
use embedded_graphics_framebuf::{backends::FrameBufferBackend, FrameBuf};
use platform::{Brightness, Platform, LED};
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

#[derive(Clone)]
struct VecFrameBufferBackend<Color: PixelColor> {
    pixels: Vec<Color>,
    size: Size,
}

impl<Color: PixelColor> VecFrameBufferBackend<Color> {
    fn new(size: Size, fill_color: Color) -> Self {
        let width = usize::try_from(size.width).unwrap();
        let height = usize::try_from(size.height).unwrap();
        let pixels = vec![fill_color; width * height];
        Self { pixels, size }
    }
}

impl<Color: PixelColor> FrameBufferBackend for &mut VecFrameBufferBackend<Color> {
    type Color = Color;

    fn set(&mut self, index: usize, color: Self::Color) {
        self.pixels[index] = color;
    }

    fn get(&self, index: usize) -> Self::Color {
        self.pixels[index]
    }

    fn nr_elements(&self) -> usize {
        usize::try_from(self.size.width).unwrap() * usize::try_from(self.size.height).unwrap()
    }
}

fn draw_loop(platform: &mut impl Platform) -> Result<()> {
    let mut rng = rand::thread_rng();
    log::info!("allocating buffers");
    let mut buffer = VecFrameBufferBackend::new(platform.lcd().bounding_box().size, Rgb565::BLACK);

    let shades_of_red: Vec<Rgb565> = (0..32).map(|v| Rgb565::new(v, 0, 0)).collect();
    const MAX_INTENSITY: i32 = 3;
    const FRAMES_PER_SHADE: usize = 10;
    let total_frames: usize = FRAMES_PER_SHADE * shades_of_red.len();

    loop {
        for (idx, &bgcolor) in shades_of_red.iter().enumerate() {
            let intensity = idx as i32 / (shades_of_red.len() as i32 / MAX_INTENSITY);

            for frame in 0..FRAMES_PER_SHADE {
                let brightness = Brightness::from({
                    let curr_frame = idx * FRAMES_PER_SHADE + frame;
                    let linear: f32 = curr_frame as f32 / total_frames as f32;
                    // Brightness of real TFT LEDs is *very* non-linear. Event a tiny amount of
                    // PWM duty (that we map this brightness to) makes them shine relatively
                    // bright, and increasing that value has somewhat less noticeable effect.
                    linear.powf(3.0)
                });
                platform.led0().set_brightness(brightness)?;
                platform.led1().set_brightness(brightness)?;

                let size = buffer.size.clone();
                let mut framebuffer =
                    FrameBuf::new(&mut buffer, size.width.try_into()?, size.height.try_into()?);

                framebuffer
                    .clear(bgcolor)
                    .context("DrawTarget::clear failed")?;
                Text::with_alignment(
                    "Analyzing Android.bp...",
                    intensify(&mut rng, platform.lcd().bounding_box().center(), intensity),
                    MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE),
                    Alignment::Center,
                )
                .draw(&mut framebuffer)
                .context("Drawable::draw failed")?;

                let bb = platform.lcd().bounding_box();
                platform
                    .lcd()
                    .fill_contiguous(&bb, buffer.pixels.iter().copied())
                    .map_err(|_| anyhow::Error::msg("DrawTarget::fill_contiguous failed"))?;

                platform.sleep(Duration::from_millis(10));
            }
        }
    }
}

fn main() {
    #[cfg(target_arch = "xtensa")]
    let mut platform = platform::new_esp32().expect("platform::new_esp32 failed");
    #[cfg(target_os = "linux")]
    let mut platform = platform::new_pc().expect("platform::new_pc failed");

    loop {
        match draw_loop(&mut platform) {
            Ok(_) => log::warn!("draw_loop exited with success (?)"),
            Err(e) => log::error!("draw_loop exited with error: {:?}", e),
        }
    }
}
