use std::{ops::{Div, Range, Rem}, time::{Duration, Instant}};

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

#[derive(Clone, Copy, Debug)]
struct RowOffset {
    offset: usize,
    row_width: usize,
}

impl RowOffset {
    fn new<C: PixelColor, B: FrameBufferBackend<Color = C>>(offset: usize, fb: &FrameBuf<C, B>) -> Self {
        assert!(fb.width() > 0);
        let row_width = fb.width();
        let offset = offset.min(row_width);
        Self { offset, row_width }
    }

    fn range_to(self, other: usize) -> RowRange {
        RowRange {
            start: self.offset.min(other).min(self.row_width),
            end: self.offset.max(other).min(self.row_width),
            row_width: self.row_width,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct RowRange {
    start: usize,
    end: usize,
    row_width: usize,
}

impl RowRange {
    fn offset(self, rhs: isize) -> RowRange {
        let start = ((self.start as isize).saturating_add(rhs).max(0) as usize).min(self.row_width);
        let end = ((self.end as isize).saturating_add(rhs).max(0) as usize).min(self.row_width);
        Self { start, end, ..self }
    }

    fn to_range(&self) -> Range<usize> {
        self.start..self.end
    }
}

struct Intensity(usize);

impl Intensity {
    const MAX: Intensity = Intensity(128);
}

impl From<usize> for Intensity {
    fn from(value: usize) -> Self {
        Self(value.min(Self::MAX.0))
    }
}

fn add_noise<B: FrameBufferBackend<Color = Rgb565>>(fb: &mut FrameBuf<Rgb565, B>, rng: &mut impl Rng, intensity: Intensity) {
    for index in 0..fb.data.nr_elements() {
        let apply_noise = rng.next_u32() as usize % Intensity::MAX.0 < intensity.0;
        if apply_noise {
            let random_color = Rgb565::new((rng.next_u32() % 32) as u8, (rng.next_u32() % 64) as u8, (rng.next_u32() % 32) as u8);
            fb.data.set(index, random_color);
        }
    }
}

fn glitch<C: PixelColor, B: FrameBufferBackend<Color = C>>(fb: &mut FrameBuf<C, B>, rng: &mut impl Rng, max_offset: usize) {
    if max_offset == 0 {
        return;
    }

    for line in 0..fb.height() {
        let should_glitch = rng.next_u32() % 128 < 32;
        if should_glitch {
            let mut rand_idx = || rng.next_u32() as usize % fb.width();
            let offset = (rand_idx() % max_offset) as i32 - (max_offset as i32 / 2);
            let src = RowOffset::new(rand_idx(), fb).range_to(rand_idx());
            let dst = src.offset(offset as _);

            let row_index = line * fb.width();
            if offset < 0 {
                for (src, dst) in src.to_range().zip(dst.to_range()) {
                    fb.data.set(row_index + dst, fb.data.get(row_index + src));
                }
            } else {
                for (src, dst) in src.to_range().zip(dst.to_range()).rev() {
                    fb.data.set(row_index + dst, fb.data.get(row_index + src));
                }
            }
        }
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
        let mut glitchiness = 0;

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
                    glitchiness += 1;
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

                glitch(&mut framebuffer, &mut rng, glitchiness as usize);

                let bb = platform.lcd().bounding_box();
                platform
                    .lcd()
                    .fill_contiguous(&bb, buffer.pixels.iter().copied())
                    .map_err(|_| anyhow::Error::msg("DrawTarget::fill_contiguous failed"))?;

                platform.sleep(Duration::from_millis(10));
            }
        }

        for _ in 0..FRAMES_PER_SHADE {
            let size = buffer.size.clone();
            let mut framebuffer =
                FrameBuf::new(&mut buffer, size.width.try_into()?, size.height.try_into()?);
            add_noise(&mut framebuffer, &mut rng, Intensity::MAX);

            let bb = platform.lcd().bounding_box();
            platform
                .lcd()
                .fill_contiguous(&bb, buffer.pixels.iter().copied())
                .map_err(|_| anyhow::Error::msg("DrawTarget::fill_contiguous failed"))?;

            platform.sleep(Duration::from_millis(10));
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
