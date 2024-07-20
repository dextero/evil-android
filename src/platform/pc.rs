use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Result;
use embedded_graphics::{
    geometry::Size,
    pixelcolor::{Rgb565, Rgb888, RgbColor},
    prelude::DrawTarget,
};
use embedded_graphics_framebuf::{backends::FrameBufferBackend, FrameBuf};
use glium::{backend::glutin::SimpleWindowBuilder, implement_vertex, Surface};
use slice_of_array::SliceFlatExt;
use winit::platform::wayland::EventLoopBuilderExtWayland;

use super::Brightness;

struct Rgba32FrameBufferBackend {
    pixels: Vec<[u8; 4]>,
    size: Size,
}

impl Rgba32FrameBufferBackend {
    fn new(size: Size, fill_color: Rgb565) -> Self {
        let width = usize::try_from(size.width).unwrap();
        let height = usize::try_from(size.height).unwrap();
        let pixels = vec![[fill_color.r(), fill_color.g(), fill_color.b(), 255]; width * height];
        Self {
            pixels,
            size,
        }
    }

    fn to_gl_texture<
        T: glium::glutin::surface::SurfaceTypeTrait
            + glium::glutin::surface::ResizeableSurface
            + 'static,
    >(
        &self,
        display: &glium::Display<T>,
    ) -> Result<glium::texture::Texture2d> {
        let image = glium::texture::RawImage2d::from_raw_rgba_reversed(
            self.pixels.flat(),
            (self.size.width, self.size.height),
        );
        Ok(glium::texture::Texture2d::new(display, image)?)
    }
}

#[derive(Clone)]
struct SyncFBBackend(Arc<Mutex<Rgba32FrameBufferBackend>>);

impl FrameBufferBackend for SyncFBBackend {
    type Color = Rgb565;

    fn set(&mut self, index: usize, color: Self::Color) {
        let color = Rgb888::from(color);
        self.0.lock().unwrap().pixels[index] = [color.r(), color.g(), color.b(), 255]
    }

    fn get(&self, index: usize) -> Self::Color {
        let [r, g, b, _] = self.0.lock().unwrap().pixels[index];
        Rgb565::new(r >> 3, g >> 2, b >> 3)
    }

    fn nr_elements(&self) -> usize {
        let size = self.0.lock().unwrap().size.clone();
        usize::try_from(size.width).unwrap() * usize::try_from(size.height).unwrap()
    }
}

pub struct FakeLED(Mutex<Brightness>);

impl super::LED for FakeLED {
    fn set_brightness(&mut self, brightness: Brightness) -> Result<()> {
        *self.0.lock().unwrap() = brightness;
        Ok(())
    }
}

pub struct Platform {
    draw_target: FrameBuf<Rgb565, SyncFBBackend>,
    led0: FakeLED,
    led1: FakeLED,
}

#[derive(Clone, Copy, Default)]
struct Vertex {
    pos: [f32; 2],
    tex_coords: [f32; 2],
}

implement_vertex!(Vertex, pos, tex_coords);

pub fn new_platform() -> Result<impl crate::platform::Platform> {
    let size = Size::new(160, 128);
    let pixel_buffer = SyncFBBackend(Arc::new(Mutex::new(Rgba32FrameBufferBackend::new(
        size,
        Rgb565::BLACK,
    ))));
    let draw_target = FrameBuf::new(
        pixel_buffer.clone(),
        size.width.try_into()?,
        size.height.try_into()?,
    );
    let led0 = FakeLED(Mutex::new(0f32.into()));
    let led1 = FakeLED(Mutex::new(0f32.into()));

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

        let vs_2d_pos_src = r#"
#version 140

in vec2 pos;
in vec2 tex_coords;
out vec2 v_tex_coords;

void main() {
    gl_Position = vec4(pos, 0.0, 1.0);
    v_tex_coords = tex_coords;
}
        "#;
        let fs_color_src = r#"
#version 140

uniform vec4 u_color;

out vec4 color;

void main() {
    color = u_color;
}
        "#;
        let fs_texture_src = r#"
#version 140

uniform sampler2D u_texture;

in vec2 v_tex_coords;
out vec4 color;

void main() {
    color = texture(u_texture, v_tex_coords);
}
        "#;
        let color_2d_program =
            glium::Program::from_source(&display, vs_2d_pos_src, fs_color_src, None).unwrap();
        let texture_2d_program =
            glium::Program::from_source(&display, vs_2d_pos_src, fs_texture_src, None).unwrap();

        let circle_vertices = {
            let mut v = vec![Vertex {
                pos: [0.0, 0.0],
                ..Default::default()
            }];
            let points = 64;
            for i in 0..points {
                let angle = (2.0 * std::f64::consts::PI * i as f64 / points as f64) as f32;
                v.push(Vertex {
                    pos: [angle.cos(), angle.sin()],
                    tex_coords: Default::default(),
                });
            }
            v
        };
        let circle_vertices = glium::VertexBuffer::new(&display, &circle_vertices).unwrap();
        let lcd_vertices = vec![
            Vertex {
                pos: [-0.5, -0.5],
                tex_coords: [0.0, 0.0],
            },
            Vertex {
                pos: [0.5, -0.5],
                tex_coords: [1.0, 0.0],
            },
            Vertex {
                pos: [-0.5, 0.5],
                tex_coords: [0.0, 1.0],
            },
            Vertex {
                pos: [0.5, 0.5],
                tex_coords: [1.0, 1.0],
            },
        ];
        let lcd_vertices = glium::VertexBuffer::new(&display, &lcd_vertices).unwrap();

        let result = event_loop.run(move |event, window_target| match event {
            winit::event::Event::WindowEvent { event, .. } => match event {
                winit::event::WindowEvent::CloseRequested => window_target.exit(),
                winit::event::WindowEvent::RedrawRequested => {
                    let mut frame = display.draw();
                    const ANDROID_GREEN: Rgb888 = Rgb888::new(61, 220, 132);
                    frame.clear_color_srgb(
                        ANDROID_GREEN.r() as f32 / 255.0f32,
                        ANDROID_GREEN.g() as f32 / 255.0f32,
                        ANDROID_GREEN.b() as f32 / 255.0f32,
                        1.0f32,
                    );

                    let texture = pixel_buffer.0.lock().unwrap().to_gl_texture(&display).unwrap();
                    let uniforms = glium::uniform! {
                        u_texture: &texture,
                    };
                    frame
                        .draw(
                            &lcd_vertices,
                            glium::index::NoIndices(glium::index::PrimitiveType::TriangleStrip),
                            &texture_2d_program,
                            &uniforms,
                            &Default::default(),
                        )
                        .unwrap();

                    match frame.finish() {
                        Ok(_) => {}
                        Err(e) => log::error!("Surface::finish failed: {e:?}"),
                    }
                }
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

    Ok(Platform {
        draw_target,
        led0,
        led1,
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
