use std::time::Duration;

use anyhow::Result;
use embedded_graphics::draw_target::DrawTarget;

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Brightness(f32);

impl From<f32> for Brightness {
    fn from(value: f32) -> Self {
        Ok(Brightness(value.clamp(0.0f32, 1.0f32)))
    }
}

pub trait LED {
    fn set_brightness(&mut self, brightness: Brightness) -> Result<()>;
}

pub trait Platform {
    const LCD_SIZE: Size;
    const LCD_PIXEL_COUNT: usize = (LCD_SIZE.width * LCD_SIZE.height) as usize;

    fn sleep(&mut self, duration: Duration);
    fn lcd(&mut self) -> &mut impl DrawTarget;
    fn led0(&mut self) -> &mut impl LED;
    fn led1(&mut self) -> &mut impl LED;
}

mod esp32;
pub use esp32::new_platform as new_esp32;
