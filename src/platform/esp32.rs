use std::time::Duration;

use anyhow::{Context, Result};
use embedded_graphics::{draw_target::DrawTarget, geometry::Size, pixelcolor::Rgb565};
use esp_idf_hal::ledc::{config::TimerConfig, LedcDriver, LedcTimerDriver, LEDC};
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
use st7735_lcd::ST7735;

use super::{Brightness, LED};

impl LED for LedcDriver<'_> {
    fn set_brightness(&mut self, brightness: Brightness) -> Result<()> {
        let led_duty = (f32::from(brightness) * self.get_max_duty() as f32) as u32;
        Ok(self.set_duty(led_duty)?)
    }
}

pub struct Platform<Lcd: DrawTarget<Color = Rgb565>, Led0Pin: LED, Led1Pin: LED> {
    lcd: Lcd,
    led0: Led0Pin,
    led1: Led1Pin,
}

pub fn new_platform() -> Result<impl super::Platform> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    let Peripherals {
        spi2: lcd_spi,
        ledc:
            LEDC {
                timer0: led_timer,
                channel0: led_channel0,
                channel1: led_channel1,
                ..
            },
        pins:
            Pins {
                gpio13: lcd_spi_mosi,
                gpio14: lcd_spi_scl,
                gpio15: lcd_spi_cs,
                gpio16: lcd_reset,
                gpio17: lcd_a0,
                gpio18: lcd_led,
                gpio19: led_pin0,
                gpio21: led_pin1,
                ..
            },
        ..
    } = Peripherals::take().context("Peripherals::take failed")?;

    let timer_config = TimerConfig::default().frequency(5000.Hz().into());
    let ledc_timer =
        LedcTimerDriver::new(led_timer, &timer_config).context("LedcTimerDriver::new failed")?;
    let led0 = LedcDriver::new(led_channel0, &ledc_timer, led_pin0)
        .context("LedcDriver::new failed for LED0")?;
    let led1 = LedcDriver::new(led_channel1, &ledc_timer, led_pin1)
        .context("LedcDriver::new faled for LED1")?;

    let lcd_spi = SpiDeviceDriver::new_single(
        lcd_spi,
        lcd_spi_scl,
        lcd_spi_mosi,
        <Option<AnyInputPin>>::None,
        Some(lcd_spi_cs),
        &SpiDriverConfig::new(),
        &Config::new().baudrate(26.MHz().into()).data_mode(MODE_3),
    )
    .context("SpiDeviceDriver::new_single failed")?;
    let lcd_reset = PinDriver::output(lcd_reset.downgrade_output())
        .context("PinDriver::output failed for lcd_reset")?;
    let lcd_a0 = PinDriver::output(lcd_a0.downgrade_output())
        .context("PinDriver::output failed for lcd_a0")?;
    let mut lcd_led = PinDriver::output(lcd_led.downgrade_output())
        .context("PinDriver::output failed for lcd_led")?;

    const LCD_SIZE: Size = Size::new(160, 128);
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
    lcd.init(&mut FreeRtos)
        .map_err(|_| anyhow::Error::msg("ST7735::init failed"))?;
    lcd.set_orientation(&st7735_lcd::Orientation::Landscape)
        .map_err(|_| anyhow::Error::msg("ST7735::set_orientation failed"))?;
    lcd_led
        .set_high()
        .context("PinDriver::set_high failed for lcd_led")?;

    let platform = Platform { lcd, led0, led1 };
    Ok(platform)
}

impl<Lcd: DrawTarget<Color = Rgb565>, Led0Pin: LED, Led1Pin: LED> super::Platform for Platform<Lcd, Led0Pin, Led1Pin> {
    fn sleep(&mut self, duration: Duration) {
        FreeRtos::delay_ms(
            duration
                .as_millis()
                .try_into()
                .expect("can't sleep for more than u32::MAX_VALUE ms"),
        );
    }

    fn lcd(&mut self) -> &mut impl DrawTarget<Color = Rgb565> {
        &mut self.lcd
    }

    fn led0(&mut self) -> &mut impl LED {
        &mut self.led0
    }

    fn led1(&mut self) -> &mut impl LED {
        &mut self.led1
    }
}
