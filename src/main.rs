mod cli;
mod physics;
mod render;
mod types;

#[cfg(feature = "capture")]
mod capture;

use std::sync::Arc;

use clap::Parser;
use glam::Vec2;
use log::warn;
use rand::Rng;
use winit::{
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

use crate::{physics::PhysicsModule, render::RenderModule, types::Particle};

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    // Collect Arguments
    let args = cli::Args::parse();

    // Setup Winit
    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(WindowBuilder::new().build(&event_loop).unwrap());
    let size = window.inner_size();
    event_loop.set_control_flow(ControlFlow::Poll);

    // Setup Wgpu
    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(window.clone()).unwrap();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        })
        .await
        .expect("Failed to find an appropriate adapter");
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        )
        .await
        .expect("Failed to create Device");

    // Shader
    let num_particles: u32 = args.particles; // NOTE: Must be a multiple of `64`
    let work_group_count = num_particles / 64;

    let physics_module = PhysicsModule::new(&device, num_particles as usize, args.gravity);
    let render_module = RenderModule::new(&device, &surface, &adapter);

    #[cfg(feature = "capture")]
    let mut capture_module = capture::CaptureModule::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        size.width,
        size.height,
    );

    let mut rng = rand::thread_rng();
    // Generate Chunks of Random Particles
    for c in 0..num_particles as u64 / 128 {
        let chunk = Vec2::new(rng.gen_range(-20f32..=20f32), rng.gen_range(-20f32..=20f32));
        for p in 0..128 as u64 {
            let dir = Vec2::new(rng.gen_range(-1f32..=1f32), rng.gen_range(-1f32..=1f32));
            let d = rng.gen_range(0.0..=4.0);
            let particle = Particle {
                position: chunk + dir * d,
                velocity: Vec2::ZERO,
                radius: 0.1,
                mass: 0.1,
            };

            let i = c + p * (num_particles as u64 / 128);
            queue.write_buffer(
                &physics_module.particle_buffer_collision,
                i * 24,
                bytemuck::bytes_of(&particle),
            );
        }
    }
    // Generate Random Particles
    // for i in 0..NUM_PARTICLES as u64 {
    //     let pos = Vec2::new(rng.gen_range(-20f32..=20f32), rng.gen_range(-20f32..=20f32));
    //     let particle = Particle {
    //         position: pos,
    //         velocity: Vec2::ZERO,
    //         radius: 0.1, //rng.gen_range(0.01..=0.2f32);
    //         mass: 0.1, //rng.gen_range(0.01..=0.2f32);
    //     };

    //     queue.write_buffer(
    //         &physics_module.particle_buffer_collision,
    //         i * 24,
    //         bytemuck::bytes_of(&particle),
    //     );
    // }

    // Configure Surface
    let mut config = surface
        .get_default_config(&adapter, size.width, size.height)
        .unwrap();
    surface.configure(&device, &config);

    render_module.update_all(&queue, size.width, size.height, 0.0, 0.0, 1.0);

    // State
    let mut is_right_click_pressed = false;
    let mut mouse_position = Vec2::ZERO;

    let mut view_offset = Vec2::new(0.0, 0.0);
    let mut view_zoom = 1.0;

    let mut is_paused = true;
    let time_scale = 100.0;
    let mut instant = std::time::Instant::now();

    // Main Loop
    event_loop
        .run(move |event, elwt| {
            let _ = (&instance, &adapter);

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    elwt.exit();
                }

                Event::WindowEvent {
                    event: WindowEvent::Resized(new_size),
                    ..
                } => {
                    config.width = new_size.width;
                    config.height = new_size.height;
                    surface.configure(&device, &config);

                    render_module.update_size(&queue, config.width, config.height);

                    #[cfg(feature = "capture")]
                    capture_module.resize(
                        &device,
                        wgpu::TextureFormat::Rgba8UnormSrgb,
                        config.width,
                        config.height,
                    );
                }

                Event::WindowEvent {
                    event: WindowEvent::KeyboardInput { event, .. },
                    ..
                } => match (event.state, event.physical_key) {
                    (ElementState::Pressed, PhysicalKey::Code(KeyCode::Space)) => {
                        is_paused = !is_paused;
                    }
                    (ElementState::Pressed, PhysicalKey::Code(KeyCode::F11)) => {
                        if window.fullscreen().is_none() {
                            window
                                .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                        } else {
                            window.set_fullscreen(None);
                        }
                    }

                    #[cfg(feature = "capture")]
                    (ElementState::Pressed, PhysicalKey::Code(KeyCode::KeyC)) => {
                        capture_module.enabled = !capture_module.enabled;
                    }

                    _ => (),
                },

                Event::WindowEvent {
                    event: WindowEvent::MouseWheel { delta, .. },
                    ..
                } => {
                    let delta = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                    } * 0.005
                        * view_zoom;

                    view_zoom = (view_zoom + delta).clamp(0.01, 10.0);
                    render_module.update_zoom(&queue, view_zoom);
                }
                Event::WindowEvent {
                    event: WindowEvent::MouseInput { state, button, .. },
                    ..
                } => match (state, button) {
                    // (ElementState::Pressed, MouseButton::Left) => is_left_click_pressed = true,
                    // (ElementState::Released, MouseButton::Left) => is_left_click_pressed = false,
                    (ElementState::Pressed, MouseButton::Right) => is_right_click_pressed = true,
                    (ElementState::Released, MouseButton::Right) => is_right_click_pressed = false,
                    _ => (),
                },
                Event::WindowEvent {
                    event: WindowEvent::CursorMoved { position, .. },
                    ..
                } => {
                    let position = Vec2::new(position.x as f32, position.y as f32);
                    if is_right_click_pressed {
                        let delta = position - mouse_position;
                        view_offset += delta * Vec2::new(1.0, -1.0) * 0.005 / view_zoom;

                        render_module.update_offset(&queue, view_offset.x, view_offset.y);
                    }

                    mouse_position = position;
                }

                Event::AboutToWait => {
                    let delta_time = if let Some(frame_time) =
                        args.framerate.map(|f| 1f32 / f as f32)
                    {
                        while instant.elapsed().as_secs_f32() < frame_time {
                            let left = frame_time - instant.elapsed().as_secs_f32();
                            if left < 0.00025 {
                                continue;
                            }

                            std::thread::sleep(std::time::Duration::from_secs_f32(left * 0.9));
                        }

                        // If we have a limited frame time we should always assume the last frame took `frame_time` seconds
                        frame_time
                    } else {
                        if capture_module.enabled == true {
                            capture_module.enabled = false;
                            warn!("The `capture` module can't run without a limited framerate.");
                        }

                        instant.elapsed().as_secs_f32()
                    };

                    physics_module.update_delta_time(&queue, delta_time * time_scale);
                    instant = std::time::Instant::now();

                    let frame = surface.get_current_texture().unwrap();
                    let mut encoder = device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                    if !is_paused {
                        let _cpass = physics_module.begin_pass(&mut encoder, work_group_count);
                    }

                    {
                        let view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());

                        render_module.begin_pass(
                            &mut encoder,
                            &view,
                            &physics_module.particle_buffer_collision,
                            num_particles,
                        );
                    }

                    #[cfg(feature = "capture")]
                    capture_module.begin_pass(
                        &mut encoder,
                        &render_module,
                        &physics_module.particle_buffer_collision,
                        num_particles,
                    );

                    #[cfg(feature = "capture")]
                    capture_module.copy_texture_to_buffer(&mut encoder);

                    queue.submit(Some(encoder.finish()));
                    frame.present();

                    #[cfg(feature = "capture")]
                    capture_module.get_frame(&device);
                }

                _ => (),
            }
        })
        .unwrap();
}
