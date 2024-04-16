mod cli;
mod follow;
mod gui;
mod particle;
mod physics;
mod render;

#[cfg(feature = "capture")]
mod capture;

use std::sync::Arc;

use clap::Parser;
use egui::Widget;
use follow::FollowModule;
use glam::Vec2;
use gui::EguiIntegration;
use log::warn;
use winit::{
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

use crate::{physics::PhysicsModule, render::RenderModule};

struct AppState {
    is_right_click_pressed: bool,
    mouse_position: Vec2,

    view_offset: Vec2,
    view_zoom: f32,

    is_paused: bool,
    framerate: u32,
    instant: std::time::Instant,
}

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

    let swapchain_capabilities = surface.get_capabilities(&adapter);
    let swapchain_format = swapchain_capabilities.formats[0];

    // Shader
    let mut num_particles: u32 = args.particles; // NOTE: Must be a multiple of `64`

    let mut physics_module = PhysicsModule::new(&device, num_particles as usize, args.gravity);
    let render_module = RenderModule::new(&device, &surface, &adapter, swapchain_format);
    let mut follow_module = FollowModule::new(&device, &physics_module.particle_buffers);

    #[cfg(feature = "capture")]
    let mut capture_module =
        capture::CaptureModule::new(&device, swapchain_format.clone(), size.width, size.height);

    particle::generate_particles(&queue, &physics_module, num_particles as u64);

    // Configure Surface
    let mut config = surface
        .get_default_config(&adapter, size.width, size.height)
        .unwrap();
    surface.configure(&device, &config);

    render_module.update_all(&queue, size.width, size.height, 0.0, 0.0, 1.0);

    // State
    let mut egui_integration =
        EguiIntegration::new(&device, swapchain_format, num_particles.to_string());

    let mut app_state = AppState {
        is_right_click_pressed: false,
        mouse_position: Vec2::ZERO,

        view_offset: Vec2::ZERO,
        view_zoom: 1.0,

        is_paused: true,
        framerate: args.framerate,
        instant: std::time::Instant::now(),
    };

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
                    egui_integration.resize(config.width, config.height);

                    #[cfg(feature = "capture")]
                    capture_module.resize(
                        &device,
                        swapchain_format.clone(),
                        config.width,
                        config.height,
                    );
                }

                Event::WindowEvent {
                    event: WindowEvent::KeyboardInput { event, .. },
                    ..
                } => {
                    let mut handled = true;
                    match (event.state, event.physical_key) {
                        (ElementState::Pressed, PhysicalKey::Code(KeyCode::Space)) => {
                            app_state.is_paused = !app_state.is_paused;
                        }
                        (ElementState::Pressed, PhysicalKey::Code(KeyCode::F11)) => {
                            if window.fullscreen().is_none() {
                                window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(
                                    None,
                                )));
                            } else {
                                window.set_fullscreen(None);
                            }
                        }

                        (ElementState::Pressed, PhysicalKey::Code(KeyCode::KeyF)) => {
                            follow_module.enabled = !follow_module.enabled;
                        }

                        #[cfg(feature = "capture")]
                        (ElementState::Pressed, PhysicalKey::Code(KeyCode::KeyC)) => {
                            capture_module.enabled = !capture_module.enabled;
                        }

                        _ => handled = false,
                    };

                    if !handled {
                        egui_integration.key_event(event);
                    }
                }

                Event::WindowEvent {
                    event: WindowEvent::MouseWheel { delta, .. },
                    ..
                } => {
                    let delta = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                    } * 0.005
                        * app_state.view_zoom;

                    app_state.view_zoom = (app_state.view_zoom + delta).clamp(0.01, 10.0);
                    render_module.update_zoom(&queue, app_state.view_zoom);
                }
                Event::WindowEvent {
                    event: WindowEvent::MouseInput { state, button, .. },
                    ..
                } => {
                    let mut handled = true;
                    match (state, button) {
                        // (ElementState::Pressed, MouseButton::Left) => is_left_click_pressed = true,
                        // (ElementState::Released, MouseButton::Left) => is_left_click_pressed = false,
                        (ElementState::Pressed, MouseButton::Right) => {
                            app_state.is_right_click_pressed = true
                        }
                        (ElementState::Released, MouseButton::Right) => {
                            app_state.is_right_click_pressed = false
                        }
                        _ => handled = false,
                    }

                    if !handled {
                        egui_integration.mouse_event(app_state.mouse_position, state, button);
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::CursorMoved { position, .. },
                    ..
                } => {
                    let position = Vec2::new(position.x as f32, position.y as f32);
                    if app_state.is_right_click_pressed {
                        let delta = position - app_state.mouse_position;
                        app_state.view_offset +=
                            delta * Vec2::new(1.0, -1.0) * 0.005 / app_state.view_zoom;

                        render_module.update_offset(
                            &queue,
                            app_state.view_offset.x,
                            app_state.view_offset.y,
                        );
                    } else {
                        egui_integration.mouse_motion(position);
                    }

                    app_state.mouse_position = position;
                }

                Event::AboutToWait => {
                    if app_state.framerate > 0 {
                        let frame_time = 1.0 / app_state.framerate as f32;
                        while app_state.instant.elapsed().as_secs_f32() < frame_time {
                            let left = frame_time - app_state.instant.elapsed().as_secs_f32();
                            if left < 0.00025 {
                                continue;
                            }

                            std::thread::sleep(std::time::Duration::from_secs_f32(left * 0.9));
                        }
                    } else {
                        if capture_module.enabled == true {
                            capture_module.enabled = false;
                            warn!("The `capture` module can't run without a limited framerate.");
                        }
                    }

                    let delta_time = app_state.instant.elapsed().as_secs_f32();
                    physics_module.update_delta_time(&queue, args.time_scale);
                    app_state.instant = std::time::Instant::now();

                    let frame = surface.get_current_texture().unwrap();
                    let mut encoder = device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                    if !app_state.is_paused {
                        let _cpass = physics_module.begin_pass(&mut encoder, num_particles / 64);
                    }

                    {
                        egui_integration.run(|ctx, particle_count| {
                            egui::Window::new("Settings")
                                .default_width(145.0)
                                .show(&ctx, |ui| {
                                    let mut framerate_text = app_state.framerate.to_string();
                                    ui.checkbox(&mut app_state.is_paused, "Paused [Space]");
                                    ui.horizontal(|ui| {
                                        ui.label("Fixed FPS");
                                        ui.text_edit_singleline(&mut framerate_text);
                                    });
                                    ui.label(format!("FPS {:.1}", 1.0 / delta_time));
                                    if let Ok(new_framerate) = framerate_text.parse::<u32>() {
                                        app_state.framerate = new_framerate;
                                    }
                                });

                            egui::Window::new("Simulation")
                                .default_width(145.0)
                                .show(&ctx, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label("Particles");
                                        ui.text_edit_singleline(particle_count);
                                    });
                                    if ui.button("Regenerate").clicked() {
                                        let Ok(particle_count) = particle_count.parse::<u32>()
                                        else {
                                            return;
                                        };

                                        if particle_count % 64 > 0 {
                                            return;
                                        }

                                        if num_particles != particle_count {
                                            physics_module
                                                .resize_buffers(&device, particle_count as usize);
                                        }

                                        num_particles = particle_count;
                                        particle::generate_particles(
                                            &queue,
                                            &physics_module,
                                            num_particles as u64,
                                        );
                                    }
                                });

                            egui::Window::new("View")
                                .default_width(145.0)
                                .show(&ctx, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label("Zoom");
                                        egui::widgets::Slider::new(
                                            &mut app_state.view_zoom,
                                            0.01..=10.0,
                                        )
                                        .ui(ui);
                                    });

                                    ui.add_space(10.0);
                                    ui.heading("Follow");
                                    ui.separator();
                                    ui.checkbox(&mut follow_module.enabled, "Enabled [f]");
                                    ui.checkbox(
                                        &mut follow_module.view_center_of_mass,
                                        "Center of Mass",
                                    );
                                    ui.checkbox(&mut follow_module.view_scale, "Auto Zoom");
                                });

                            egui::Window::new("Capture")
                                .default_width(145.0)
                                .show(&ctx, |ui| {
                                    let size = window.inner_size();
                                    ui.checkbox(&mut capture_module.enabled, "Enabled [c]");
                                    ui.label(format!("Size {}x{}", size.width, size.height));
                                    ui.label(format!("Framerate: {}", app_state.framerate));

                                    if app_state.framerate == 0 {
                                        ui.separator();
                                        ui.colored_label(
                                            egui::Color32::RED,
                                            "Can't capture without a fixed framerate",
                                        );
                                    }
                                });
                        });

                        egui_integration.pre_render(&device, &queue, &mut encoder);
                    }

                    {
                        let view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());

                        let mut rpass = render_module.begin_pass(
                            &mut encoder,
                            &view,
                            physics_module.current_buffer(),
                            num_particles,
                        );

                        egui_integration.render(&mut rpass);
                    }

                    if follow_module.enabled {
                        follow_module.begin_pass(&mut encoder, physics_module.current);
                        follow_module.copy_buffer_to_buffer(&mut encoder);
                    }

                    {
                        #[cfg(feature = "capture")]
                        capture_module.begin_pass(
                            &mut encoder,
                            &render_module,
                            physics_module.current_buffer(),
                            num_particles,
                        );

                        #[cfg(feature = "capture")]
                        capture_module.copy_texture_to_buffer(&mut encoder);
                    }

                    queue.submit(Some(encoder.finish()));
                    frame.present();

                    {
                        #[cfg(feature = "capture")]
                        capture_module.get_frame(&device);
                    }

                    if follow_module.enabled {
                        if let Some([center, size]) = follow_module.get_data(&device) {
                            follow_module.center_of_mass = -center;
                            follow_module.size = size;

                            if follow_module.view_center_of_mass {
                                app_state.view_offset = -center;
                                render_module.update_offset(&queue, -center.x, -center.y);
                            }

                            if follow_module.view_scale {
                                app_state.view_zoom = size.length_recip().powf(0.75);
                                render_module.update_zoom(&queue, app_state.view_zoom);
                            }
                        }
                    }
                }

                _ => (),
            }
        })
        .unwrap();
}
