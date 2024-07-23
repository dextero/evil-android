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
        Self { pixels, size }
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

#[derive(Clone)]
pub struct FakeLED(Arc<Mutex<Brightness>>);

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
}

implement_vertex!(Vertex, pos);

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
    let led0 = FakeLED(Arc::new(Mutex::new(0f32.into())));
    let led1 = FakeLED(Arc::new(Mutex::new(0f32.into())));

    let led0_clone = led0.clone();
    let led1_clone = led1.clone();
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
            .with_inner_size(1600, 1200)
            .build(&event_loop);

        let vs_src = r#"
#version 140

in vec2 pos;

void main() {
    gl_Position = vec4(pos, 0.0, 1.0);
}
        "#;

        // https://www.shadertoy.com/view/McfcWB
        let fs_src = r#"
#version 140

uniform vec2 u_resolution;
uniform vec3 u_left_eye_color;
uniform vec3 u_right_eye_color;
uniform sampler2D u_lcd_texture;

out vec4 fragColor;

const float PI = 3.1415926535897932384626433832795;

vec2 translate(vec2 pos, vec2 delta) {
    return pos + delta;
}

vec2 rotate(vec2 pos, float angle) {
    return vec2(pos.x * cos(angle) - pos.y * sin(angle),
                pos.y * cos(angle) + pos.x * sin(angle));
}

bool in_ellipse(vec2 pos, vec2 center, vec2 radii) {
    vec2 delta = pos - center;
    delta.y /= radii.y / radii.x;
    return length(delta) < radii.x;
}

bool in_circle(vec2 pos, vec2 center, float radius) {
    return distance(pos, center) < radius;
}

bool in_rect(vec2 pos,vec2 top_left, vec2 bottom_right) {
    return !(pos.x < top_left.x || pos.x > bottom_right.x || pos.y < top_left.y || pos.y > bottom_right.y);
}

void main() {
    // Normalized pixel coordinates -200..200 on y, aspect ratio preserving on x
    vec2 pos = vec2(gl_FragCoord.x - u_resolution.x / 2.0,
                    gl_FragCoord.y - u_resolution.y / 2.0);
    pos /= u_resolution.y;
    pos *= 400.0;
    
    vec4 col_bg = vec4(1.0, 1.0, 1.0, 0.0);
    vec4 col_android = vec4(0.23921568627450981, 0.8627450980392157, 0.5176470588235295, 1.0);
    
    bool in_left_eye = in_circle(vec2(-pos.x, pos.y), vec2(42, 84), 8.0);
    bool in_right_eye = in_circle(vec2(pos.x, pos.y), vec2(42, 84), 8.0);

    float angle_rad = 29.0 * PI / 180.0;
    bool in_android_antennas = in_rect(rotate(vec2(abs(pos.x), pos.y), angle_rad), vec2(-14, 86), vec2(-14+6, 86+66));
    bool in_android_antenna_tips = in_circle(rotate(vec2(abs(pos.x), pos.y), angle_rad), vec2(-14+3, 86+66), 3.0);
    bool in_android_head_base = in_ellipse(pos, vec2(0, 41), vec2(91, 84)) && pos.y > 41.0;
    bool in_android_head = in_android_head_base || in_android_antennas || in_android_antenna_tips;
    
    bool in_android_body_upper = in_rect(pos, vec2(-91, 35-142+22), vec2(-91+182, 35));
    bool in_android_body_mid = in_rect(pos, vec2(-91+22, 35-142), vec2(-91+182-22, 35));
    bool in_android_body_lower_corners = in_circle(vec2(abs(pos.x), pos.y), vec2(91-22, 35-142+22), 22.0);
    bool in_android_body = in_android_body_upper || in_android_body_mid || in_android_body_lower_corners;
    
    bool in_android_arms_upper = in_circle(vec2(abs(pos.x), pos.y), vec2(145-24, 10), 24.0);
    bool in_android_arms_mid= in_rect(vec2(abs(pos.x), pos.y), vec2(145-48, 10-133+58), vec2(145, 10));
    bool in_android_arms_lower = in_circle(vec2(abs(pos.x), pos.y), vec2(145-24, 10-133+58), 24.0);
    bool in_android_arms = in_android_arms_upper || in_android_arms_mid || in_android_arms_lower;
    
    bool in_android_legs_mid= in_rect(vec2(abs(pos.x), pos.y), vec2(65-48, 10-133-25), vec2(65, 10-25));
    bool in_android_legs_lower = in_circle(vec2(abs(pos.x), pos.y), vec2(65-24, 10-133-25), 24.0);
    bool in_android_legs = in_android_legs_mid || in_android_legs_lower;
    
    bool in_android = in_android_head || in_android_body || in_android_arms || in_android_legs;
    
    vec2 display_center = vec2(0, -35);
    vec2 display_size = vec2(160, 128);
    float display_scale = 0.7;
    display_size *= display_scale;
    vec2 display_uv = (pos - (display_center - display_size / 2.0)) / display_size;
    bool in_display = in_rect(pos, display_center - display_size / 2.0, display_center + display_size / 2.0);

    if (in_left_eye) {
        fragColor = vec4(u_left_eye_color, 1.0);
    } else if (in_right_eye) {
        fragColor = vec4(u_right_eye_color, 1.0);
    } else if (in_display) {
        fragColor = texture2D(u_lcd_texture, display_uv);
    } else if (in_android) {
        fragColor = col_android;
    } else {
        fragColor = col_bg;
    }
}
        "#;
        let program = glium::Program::from_source(&display, vs_src, fs_src, None).unwrap();

        let vertices = vec![
            Vertex { pos: [-1.0, -1.0] },
            Vertex { pos: [1.0, -1.0] },
            Vertex { pos: [-1.0, 1.0] },
            Vertex { pos: [1.0, 1.0] },
        ];
        let vertices = glium::VertexBuffer::new(&display, &vertices).unwrap();

        let result = event_loop.run(move |event, window_target| match event {
            winit::event::Event::WindowEvent { event, .. } => match event {
                winit::event::WindowEvent::CloseRequested => window_target.exit(),
                winit::event::WindowEvent::RedrawRequested => {
                    let mut frame = display.draw();
                    frame.clear_color_srgb(1.0f32, 1.0f32, 1.0f32, 1.0f32);

                    let window_size = window.inner_size();
                    let texture = pixel_buffer
                        .0
                        .lock()
                        .unwrap()
                        .to_gl_texture(&display)
                        .unwrap();
                    let uniforms = glium::uniform! {
                        u_resolution: [window_size.width as f32, window_size.height as f32],
                        u_left_eye_color: [(*led0_clone.0.lock().unwrap()).into(), 0.0f32, 0.0f32],
                        u_right_eye_color: [(*led1_clone.0.lock().unwrap()).into(), 0.0f32, 0.0f32],
                        u_lcd_texture: &texture,
                    };
                    frame
                        .draw(
                            &vertices,
                            glium::index::NoIndices(glium::index::PrimitiveType::TriangleStrip),
                            &program,
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
        // Artificially limit FPS. The real LCD is pretty slow.
        std::thread::sleep(Duration::from_millis(10));
        &mut self.draw_target
    }

    fn led0(&mut self) -> &mut impl super::LED {
        &mut self.led0
    }

    fn led1(&mut self) -> &mut impl super::LED {
        &mut self.led1
    }
}
