use std::time::Duration;

use anyhow::Result;
use embedded_graphics::{
    geometry::Size,
    pixelcolor::{Rgb565, RgbColor},
    prelude::DrawTarget,
};
use embedded_graphics_framebuf::FrameBuf;
use glium::backend::glutin::SimpleWindowBuilder;
use winit::platform::wayland::EventLoopBuilderExtWayland;

use crate::VecFrameBufferBackend;

use super::Brightness;

pub struct FakeLED(Brightness);

impl super::LED for FakeLED {
    fn set_brightness(&mut self, brightness: Brightness) -> Result<()> {
        self.0 = brightness;
        Ok(())
    }
}

pub struct Platform {
    draw_target: FrameBuf<Rgb565, VecFrameBufferBackend<Rgb565>>,
    led0: FakeLED,
    led1: FakeLED,
}

pub fn new_platform() -> Result<impl crate::platform::Platform> {
    std::thread::spawn(move || {
        let event_loop = match winit::event_loop::EventLoopBuilder::new()
            .with_any_thread(true)
            .build()
        {
            Ok(l) => l,
            Err(e) => {
                log::error!("EventLoopBuilder::build failed: {e:?}");
                std::process::exit(1);
            }
        };
        let (window, display) = SimpleWindowBuilder::new()
            .with_title("evil-android")
            .build(&event_loop);

        let result = event_loop.run(move |event, window_target| match event {
            winit::event::Event::WindowEvent { event, .. } => match event {
                winit::event::WindowEvent::CloseRequested => window_target.exit(),
                winit::event::WindowEvent::RedrawRequested => todo!(),
                _ => {}
            },
            winit::event::Event::AboutToWait => window.request_redraw(),
            _ => {}
        });

        match result {
            Ok(_) => {
                log::info!("window closed");
                std::process::exit(0);
            }
            Err(e) => {
                log::error!("event loop terminated with error: {e:?}");
                std::process::exit(1);
            }
        }
    });

    let size = Size::new(160, 128);
    Ok(Platform {
        draw_target: FrameBuf::new(
            VecFrameBufferBackend::new(size, Rgb565::BLACK),
            size.width.try_into()?,
            size.height.try_into()?,
        ),
        led0: FakeLED(0f32.into()),
        led1: FakeLED(0f32.into()),
    })
}

impl crate::platform::Platform for Platform {
    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }

    fn lcd(&mut self) -> &mut impl DrawTarget<Color = Rgb565> {
        &mut self.draw_target
    }

    fn led0(&mut self) -> &mut impl super::LED {
        &mut self.led0
    }

    fn led1(&mut self) -> &mut impl super::LED {
        &mut self.led1
    }
}
