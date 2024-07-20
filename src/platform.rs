use std::time::Duration;

use anyhow::Result;
use embedded_graphics::{draw_target::DrawTarget, pixelcolor::Rgb565};

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Brightness(f32);

impl From<f32> for Brightness {
    fn from(value: f32) -> Self {
        Brightness(value.clamp(0.0f32, 1.0f32))
    }
}

impl From<Brightness> for f32 {
    fn from(value: Brightness) -> Self {
        value.0
    }
}

pub trait LED {
    fn set_brightness(&mut self, brightness: Brightness) -> Result<()>;
}

pub trait Platform {
    fn sleep(&mut self, duration: Duration);
    fn lcd(&mut self) -> &mut impl DrawTarget<Color = Rgb565>;
    fn led0(&mut self) -> &mut impl LED;
    fn led1(&mut self) -> &mut impl LED;
}

#[cfg(esp32)]
mod esp32;
#[cfg(esp32)]
pub use esp32::new_platform as new_esp32;
