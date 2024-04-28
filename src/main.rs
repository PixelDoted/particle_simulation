mod cli;
mod follow;
mod framepace;
mod gpu;
mod gui;
mod particle;
mod physics;
mod render;

#[cfg(feature = "capture")]
mod capture;

use std::sync::Arc;

use capture::CaptureModule;
use clap::Parser;
use egui::Widget;
use follow::FollowModule;
use framepace::Framepacer;
use glam::Vec2;
use gpu::GpuContext;
use gui::EguiIntegration;
use log::warn;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::{physics::PhysicsModule, render::RenderModule};

pub const PARTICLES_PER_WORKGROUP: u32 = 256;

struct GfxState<'a> {
    window: Arc<Window>,
    gpu: GpuContext<'a>,
    egui: EguiIntegration,

    physics_module: PhysicsModule,
    render_module: RenderModule,
    follow_module: FollowModule,
    #[cfg(feature = "capture")]
    capture_module: CaptureModule,
}

struct AppState<'a> {
    tokio_rt: tokio::runtime::Runtime,
    gfx: Option<GfxState<'a>>,
    framepace: Framepacer,

    gravity: f32,
    num_particles: u32,
    new_num_particles: u32,

    is_right_click_pressed: bool,
    mouse_position: Vec2,

    view_offset: Vec2,
    view_zoom: f32,

    time_scale: f32,
    is_paused: bool,
    step: bool,
    framerate: u32,
}

impl<'a> ApplicationHandler for AppState<'a> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );
        let window_size = window.inner_size();

        let gpu = self
            .tokio_rt
            .block_on(GpuContext::new(window.clone()))
            .unwrap();
        let surface_capabilities = gpu.surface_capabilities();
        let surface_format = surface_capabilities.formats[0];

        let buffer_particles = if self.num_particles % PARTICLES_PER_WORKGROUP > 0 {
            self.num_particles + PARTICLES_PER_WORKGROUP
                - self.num_particles % PARTICLES_PER_WORKGROUP
        } else {
            self.num_particles
        };

        let physics_module =
            PhysicsModule::new(&gpu.device, buffer_particles as usize, self.gravity);
        let render_module = RenderModule::new(&gpu.device, surface_format);
        let follow_module = FollowModule::new(&gpu.device, &physics_module.particle_buffers);

        #[cfg(feature = "capture")]
        let capture_module = capture::CaptureModule::new(
            &gpu.device,
            surface_format,
            window_size.width,
            window_size.height,
        );

        particle::generate_particles(&gpu.queue, &physics_module, self.num_particles as u64);
        render_module.update_all(
            &gpu.queue,
            window_size.width,
            window_size.height,
            0.0,
            0.0,
            1.0,
        );

        self.gfx = Some(GfxState {
            window,
            egui: EguiIntegration::new(&gpu.device, surface_format),
            gpu,

            physics_module,
            render_module,
            follow_module,
            #[cfg(feature = "capture")]
            capture_module,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(gfx) = &mut self.gfx else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::Resized(new_size) => {
                gfx.gpu.config.width = new_size.width;
                gfx.gpu.config.height = new_size.height;
                gfx.gpu.reconfigure_surface();

                gfx.render_module
                    .update_size(&gfx.gpu.queue, new_size.width, new_size.height);
                gfx.egui.resize(new_size.width, new_size.height);

                let surface_capabilities = gfx.gpu.surface_capabilities();

                #[cfg(feature = "capture")]
                gfx.capture_module.resize(
                    &gfx.gpu.device,
                    surface_capabilities.formats[0],
                    new_size.width,
                    new_size.height,
                );
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let mut handled = true;
                match (event.state, event.physical_key) {
                    (ElementState::Pressed, PhysicalKey::Code(KeyCode::Space)) => {
                        self.is_paused = !self.is_paused;
                    }
                    (ElementState::Pressed, PhysicalKey::Code(KeyCode::F11)) => {
                        if gfx.window.fullscreen().is_none() {
                            gfx.window
                                .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                        } else {
                            gfx.window.set_fullscreen(None);
                        }
                    }

                    (ElementState::Pressed, PhysicalKey::Code(KeyCode::KeyN)) => {
                        self.step = true;
                    }

                    (ElementState::Pressed, PhysicalKey::Code(KeyCode::KeyF)) => {
                        gfx.follow_module.enabled = !gfx.follow_module.enabled;
                    }

                    #[cfg(feature = "capture")]
                    (ElementState::Pressed, PhysicalKey::Code(KeyCode::KeyC)) => {
                        gfx.capture_module.enabled = !gfx.capture_module.enabled;
                    }

                    _ => handled = false,
                };

                if !handled {
                    gfx.egui.key_event(event);
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                gfx.egui.modifiers_event(modifiers);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                } * 0.005
                    * self.view_zoom;

                self.view_zoom = (self.view_zoom + delta).clamp(0.01, 10.0);
                gfx.render_module
                    .update_zoom(&gfx.gpu.queue, self.view_zoom);
            }
            WindowEvent::MouseInput { state, button, .. } => match (state, button) {
                (ElementState::Pressed, MouseButton::Right) => self.is_right_click_pressed = true,
                (ElementState::Released, MouseButton::Right) => self.is_right_click_pressed = false,
                (state, button) => gfx.egui.mouse_event(self.mouse_position, state, button),
            },
            WindowEvent::CursorMoved { position, .. } => {
                let position = Vec2::new(position.x as f32, position.y as f32);
                if self.is_right_click_pressed {
                    let delta = position - self.mouse_position;
                    self.view_offset += delta * Vec2::new(1.0, -1.0) * 0.005 / self.view_zoom;

                    gfx.render_module.update_offset(
                        &gfx.gpu.queue,
                        self.view_offset.x,
                        self.view_offset.y,
                    );
                }

                gfx.egui.mouse_motion(position);
                self.mouse_position = position;
            }

            _ => (),
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        let Some(gfx) = &mut self.gfx else {
            return;
        };

        if gfx.capture_module.enabled && self.framerate == 0 {
            gfx.capture_module.enabled = false;
            warn!("The `capture` module can't run without a limited framerate.");
        }

        gfx.physics_module
            .update_delta_time(&gfx.gpu.queue, self.time_scale);
        self.framepace.begin_frame();

        let frame = gfx.gpu.surface.get_current_texture().unwrap();
        let mut encoder = gfx
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        if !self.is_paused || self.step {
            let _cpass = gfx
                .physics_module
                .begin_pass(&mut encoder, self.num_particles / PARTICLES_PER_WORKGROUP);

            self.step = false;
        }

        {
            gfx.egui.run(|ctx| {
                egui::Window::new("Settings")
                    .default_width(145.0)
                    .show(ctx, |ui| {
                        ui.checkbox(&mut self.is_paused, "Paused [Space]");
                        egui::DragValue::new(&mut self.framerate)
                            .suffix(" Fixed FPS")
                            .ui(ui);

                        ui.label(format!("FPS {:.1}", self.framepace.framerate()));
                    });

                egui::Window::new("Simulation")
                    .default_width(145.0)
                    .show(ctx, |ui| {
                        ui.label(format!(
                            "Center of Mass\nx: {}\ny: {}",
                            gfx.follow_module.info.center_of_mass.x,
                            gfx.follow_module.info.center_of_mass.y,
                        ));
                        ui.add_space(5.0);
                        ui.label(format!(
                            "Avg Velocity\nx: {}\ny: {}",
                            gfx.follow_module.info.avg_velocity.x,
                            gfx.follow_module.info.avg_velocity.y,
                        ));
                        ui.add_space(5.0);

                        ui.separator();
                        egui::DragValue::new(&mut self.new_num_particles)
                            .suffix(" Particles")
                            .ui(ui);

                        if ui.button("Regenerate").clicked() && self.new_num_particles > 0 {
                            if self.num_particles != self.new_num_particles {
                                let buffer_particles =
                                    if self.new_num_particles % PARTICLES_PER_WORKGROUP > 0 {
                                        self.new_num_particles + PARTICLES_PER_WORKGROUP
                                            - self.new_num_particles % PARTICLES_PER_WORKGROUP
                                    } else {
                                        self.new_num_particles
                                    };

                                gfx.physics_module
                                    .resize_buffers(&gfx.gpu.device, buffer_particles as usize);
                            }

                            self.num_particles = self.new_num_particles;
                            particle::generate_particles(
                                &gfx.gpu.queue,
                                &gfx.physics_module,
                                self.num_particles as u64,
                            );
                        }
                    });

                egui::Window::new("View")
                    .default_width(145.0)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Zoom");
                            egui::widgets::Slider::new(&mut self.view_zoom, 0.01..=10.0).ui(ui);
                        });

                        ui.add_space(10.0);
                        ui.heading("Follow");
                        ui.separator();
                        ui.checkbox(&mut gfx.follow_module.enabled, "Enabled [f]");
                        ui.checkbox(&mut gfx.follow_module.center_of_mass, "Center of Mass");
                        ui.checkbox(&mut gfx.follow_module.auto_zoom, "Auto Zoom");
                    });

                egui::Window::new("Capture")
                    .default_width(145.0)
                    .show(ctx, |ui| {
                        let size = gfx.window.inner_size();
                        ui.checkbox(&mut gfx.capture_module.enabled, "Enabled [c]");
                        ui.label(format!("Size {}x{}", size.width, size.height));
                        ui.label(format!("Framerate: {}", self.framerate));

                        if self.framerate == 0 {
                            ui.separator();
                            ui.colored_label(
                                egui::Color32::RED,
                                "Can't capture without a fixed framerate",
                            );
                        }
                    });
            });

            gfx.egui.pre_render(
                &gfx.gpu.device,
                &gfx.gpu.queue,
                &mut encoder,
                self.framepace.frametime(),
            );
        }

        {
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let mut rpass = gfx.render_module.begin_pass(
                &mut encoder,
                &view,
                gfx.physics_module.current_buffer(),
                self.num_particles,
            );

            gfx.egui.render(&mut rpass);
        }

        if gfx.follow_module.enabled {
            gfx.follow_module
                .begin_pass(&mut encoder, gfx.physics_module.current);
            gfx.follow_module.copy_buffer_to_buffer(&mut encoder);
        }

        #[cfg(feature = "capture")]
        {
            gfx.capture_module.begin_pass(
                &mut encoder,
                &gfx.render_module,
                gfx.physics_module.current_buffer(),
                self.num_particles,
            );

            gfx.capture_module.copy_texture_to_buffer(&mut encoder);
        }

        gfx.gpu.queue.submit(Some(encoder.finish()));
        frame.present();

        {
            #[cfg(feature = "capture")]
            gfx.capture_module.get_frame(&gfx.gpu.device);
        }

        if gfx.follow_module.enabled {
            if let Some(output) = gfx.follow_module.get_data(&gfx.gpu.device) {
                gfx.follow_module.info = output;

                if gfx.follow_module.center_of_mass {
                    self.view_offset = -output.center_of_mass;
                    gfx.render_module.update_offset(
                        &gfx.gpu.queue,
                        self.view_offset.x,
                        self.view_offset.y,
                    );
                }

                if gfx.follow_module.auto_zoom {
                    let size = (gfx.follow_module.info.max_position
                        - gfx.follow_module.info.min_position)
                        .abs();

                    self.view_zoom = size.length_recip().powf(0.75);
                    gfx.render_module
                        .update_zoom(&gfx.gpu.queue, self.view_zoom);
                }
            }
        }

        self.framepace.end_frame(1.0 / self.framerate as f32);
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    // Collect Arguments
    let args = cli::Args::parse();

    // Setup Winit
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    // State
    let mut app_state = AppState {
        tokio_rt: tokio::runtime::Runtime::new()?,
        gfx: None,
        framepace: Framepacer::new(),

        gravity: args.gravity,
        num_particles: args.particles,
        new_num_particles: args.particles,

        is_right_click_pressed: false,
        mouse_position: Vec2::ZERO,

        view_offset: Vec2::ZERO,
        view_zoom: 1.0,

        time_scale: args.time_scale,
        is_paused: true,
        step: false,
        framerate: args.framerate,
    };

    event_loop.run_app(&mut app_state)?;
    Ok(())
}
