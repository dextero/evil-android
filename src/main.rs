use std::{ops::{Div, Rem}, time::{Duration, Instant}};

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

fn div_rem<T: Div<Output = T> + Rem<Output = T> + Copy>(a: T, b: T) -> (T, T) {
    (a / b, a % b)
}

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    let (mins, secs) = div_rem(secs, 60);
    let (hrs, mins) = div_rem(mins, 60);
    let (days, hrs) = div_rem(hrs, 24);
    let (years, days) = div_rem(days, 365);

    let mut s = String::new();
    if years > 0 { s = format!("{s}{years}y "); }
    if days > 0 { s = format!("{s}{days}d "); }
    if hrs > 0 { s = format!("{s}{hrs}:"); }
    format!("{s}{mins:02}:{secs:02}")
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
    const FRAMES_PER_SHADE: usize = 16;
    const UNEXAGGERATED_TIME_FRAMES: usize = FRAMES_PER_SHADE * 8;
    const EXAGGERATION_BASE: f64 = 1.01f64;
    const EXAGGERATION_FACTOR: f64 = 1.4f64;
    let total_frames: usize = FRAMES_PER_SHADE * shades_of_red.len();

    loop {
        let start_time = Instant::now();

        for (idx, &bgcolor) in shades_of_red.iter().enumerate() {
            let curr_time = Instant::now();
            let intensity = idx as i32 / (shades_of_red.len() as i32 / MAX_INTENSITY);

            for frame in 0..FRAMES_PER_SHADE {
                let curr_frame = idx * FRAMES_PER_SHADE + frame;
                let exaggeration = if curr_frame < UNEXAGGERATED_TIME_FRAMES {
                    0f64
                } else {
                    let v = curr_frame.saturating_sub(UNEXAGGERATED_TIME_FRAMES) as f64;
                    EXAGGERATION_BASE.powf(v.powf(EXAGGERATION_FACTOR))
                };
                let exaggerated_str = if exaggeration < 1e15 {
                    let exaggerated_time = (curr_time - start_time) + Duration::from_secs_f64(exaggeration);
                    format_duration(exaggerated_time)
                } else {
                    "9999999999999999999999999999".to_owned()
                };

                let brightness = Brightness::from({
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
                    &format!("{}\nAnalyzing Android.bp...", exaggerated_str),
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
