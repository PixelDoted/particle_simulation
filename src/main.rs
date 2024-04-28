mod cli;
mod follow;
mod framepace;
mod gpu;
mod gui;
mod particle;
mod physics;
mod render;
mod utils;

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
use utils::{multiple_of, Exists};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::{physics::PhysicsModule, render::RenderModule};

pub const PARTICLES_PER_WORKGROUP: u32 = 256;

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
        gpu: Exists::None,
        gfx: Exists::None,
        sim: SimulationState {
            physics_module: Exists::None,
            follow_module: Exists::None,

            gravity: args.gravity,
            particles: args.particles,

            edited_gravity: args.gravity,
            edited_particles: args.particles,
        },
        framepace: Framepacer::new(),

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

struct GfxState {
    window: Arc<Window>,
    egui: EguiIntegration,

    render_module: RenderModule,
    #[cfg(feature = "capture")]
    capture_module: CaptureModule,
}

struct SimulationState {
    physics_module: Exists<PhysicsModule>,
    follow_module: Exists<FollowModule>,

    gravity: f32,
    particles: u32,

    edited_gravity: f32,
    edited_particles: u32,
}

struct AppState<'a> {
    tokio_rt: tokio::runtime::Runtime,
    gpu: Exists<GpuContext<'a>>,
    gfx: Exists<GfxState>,
    sim: SimulationState,
    framepace: Framepacer,

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

        let buffer_particles = multiple_of(self.sim.particles, PARTICLES_PER_WORKGROUP);

        let physics_module =
            PhysicsModule::new(&gpu.device, buffer_particles as usize, self.sim.gravity);
        let render_module = RenderModule::new(&gpu.device, surface_format);
        let follow_module = FollowModule::new(&gpu.device, &physics_module.particle_buffers);

        #[cfg(feature = "capture")]
        let capture_module = capture::CaptureModule::new(
            &gpu.device,
            surface_format,
            window_size.width,
            window_size.height,
        );

        particle::generate_particles(&gpu.queue, &physics_module, self.sim.particles as u64);
        render_module.update_all(
            &gpu.queue,
            window_size.width,
            window_size.height,
            0.0,
            0.0,
            1.0,
        );

        self.gfx = Exists::Some(GfxState {
            window,
            egui: EguiIntegration::new(&gpu.device, surface_format),

            render_module,
            #[cfg(feature = "capture")]
            capture_module,
        });
        self.sim.physics_module = Exists::Some(physics_module);
        self.sim.follow_module = Exists::Some(follow_module);
        self.gpu = Exists::Some(gpu);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if self.gfx.is_none() {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::Resized(new_size) => {
                self.gpu.config.width = new_size.width;
                self.gpu.config.height = new_size.height;
                self.gpu.reconfigure_surface();

                self.gfx.render_module.update_size(
                    &self.gpu.queue,
                    new_size.width,
                    new_size.height,
                );
                self.gfx.egui.resize(new_size.width, new_size.height);

                let surface_capabilities = self.gpu.surface_capabilities();

                #[cfg(feature = "capture")]
                self.gfx.capture_module.resize(
                    &self.gpu.device,
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
                        if self.gfx.window.fullscreen().is_none() {
                            self.gfx
                                .window
                                .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                        } else {
                            self.gfx.window.set_fullscreen(None);
                        }
                    }

                    (ElementState::Pressed, PhysicalKey::Code(KeyCode::KeyN)) => {
                        self.step = true;
                    }

                    (ElementState::Pressed, PhysicalKey::Code(KeyCode::KeyF)) => {
                        self.sim.follow_module.enabled = !self.sim.follow_module.enabled;
                    }

                    #[cfg(feature = "capture")]
                    (ElementState::Pressed, PhysicalKey::Code(KeyCode::KeyC)) => {
                        self.gfx.capture_module.enabled = !self.gfx.capture_module.enabled;
                    }

                    _ => handled = false,
                };

                if !handled {
                    self.gfx.egui.key_event(event);
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.gfx.egui.modifiers_event(modifiers);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                } * 0.005
                    * self.view_zoom;

                self.view_zoom = (self.view_zoom + delta).clamp(0.01, 10.0);
                self.gfx
                    .render_module
                    .update_zoom(&self.gpu.queue, self.view_zoom);
            }
            WindowEvent::MouseInput { state, button, .. } => match (state, button) {
                (ElementState::Pressed, MouseButton::Right) => self.is_right_click_pressed = true,
                (ElementState::Released, MouseButton::Right) => self.is_right_click_pressed = false,
                (state, button) => self
                    .gfx
                    .egui
                    .mouse_event(self.mouse_position, state, button),
            },
            WindowEvent::CursorMoved { position, .. } => {
                let position = Vec2::new(position.x as f32, position.y as f32);
                if self.is_right_click_pressed {
                    let delta = position - self.mouse_position;
                    self.view_offset += delta * Vec2::new(1.0, -1.0) * 0.005 / self.view_zoom;

                    self.gfx.render_module.update_offset(
                        &self.gpu.queue,
                        self.view_offset.x,
                        self.view_offset.y,
                    );
                }

                self.gfx.egui.mouse_motion(position);
                self.mouse_position = position;
            }

            _ => (),
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.gpu.is_none()
            || self.sim.physics_module.is_none()
            || self.sim.follow_module.is_none()
        {
            return;
        }

        if self.gfx.capture_module.enabled && self.framerate == 0 {
            self.gfx.capture_module.enabled = false;
            warn!("The `capture` module can't run without a limited framerate.");
        }

        self.sim
            .physics_module
            .update_delta_time(&self.gpu.queue, self.time_scale);
        self.framepace.begin_frame();

        let frame = self.gpu.surface.get_current_texture().unwrap();
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        if !self.is_paused || self.step {
            let _cpass = self
                .sim
                .physics_module
                .begin_pass(&mut encoder, self.sim.particles / PARTICLES_PER_WORKGROUP);

            self.step = false;
        }

        if let Exists::Some(gfx) = &mut self.gfx {
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
                            self.sim.follow_module.info.center_of_mass.x,
                            self.sim.follow_module.info.center_of_mass.y,
                        ));
                        ui.add_space(5.0);
                        ui.label(format!(
                            "Avg Velocity\nx: {}\ny: {}",
                            self.sim.follow_module.info.avg_velocity.x,
                            self.sim.follow_module.info.avg_velocity.y,
                        ));
                        ui.add_space(5.0);

                        ui.separator();
                        egui::DragValue::new(&mut self.sim.edited_gravity)
                            .suffix(" Gravity")
                            .ui(ui);
                        egui::DragValue::new(&mut self.sim.edited_particles)
                            .suffix(" Particles")
                            .ui(ui);

                        if ui.button("Apply").clicked()
                            && self.sim.edited_particles > 0
                            && self.sim.edited_gravity > 0.0
                        {
                            if self.sim.particles != self.sim.edited_particles {
                                let buffer_particles =
                                    multiple_of(self.sim.edited_particles, PARTICLES_PER_WORKGROUP);

                                self.sim
                                    .physics_module
                                    .resize_buffers(&self.gpu.device, buffer_particles as usize);

                                self.sim.particles = self.sim.edited_particles;
                                particle::generate_particles(
                                    &self.gpu.queue,
                                    &self.sim.physics_module,
                                    self.sim.particles as u64,
                                );
                            }

                            if self.sim.gravity != self.sim.edited_gravity {
                                self.sim.gravity = self.sim.edited_gravity;
                                self.sim.physics_module.update_gravitational_constant(
                                    &self.gpu.queue,
                                    self.sim.gravity,
                                );
                            }
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
                        ui.checkbox(&mut self.sim.follow_module.enabled, "Enabled [f]");
                        ui.checkbox(&mut self.sim.follow_module.center_of_mass, "Center of Mass");
                        ui.checkbox(&mut self.sim.follow_module.auto_zoom, "Auto Zoom");
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
                &self.gpu.device,
                &self.gpu.queue,
                &mut encoder,
                self.framepace.frametime(),
            );

            // Render
            {
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut rpass = gfx.render_module.begin_pass(
                    &mut encoder,
                    &view,
                    self.sim.physics_module.current_buffer(),
                    self.sim.particles,
                );

                gfx.egui.render(&mut rpass);
            }

            // Capture
            #[cfg(feature = "capture")]
            {
                gfx.capture_module.begin_pass(
                    &mut encoder,
                    &gfx.render_module,
                    self.sim.physics_module.current_buffer(),
                    self.sim.particles,
                );

                gfx.capture_module.copy_texture_to_buffer(&mut encoder);
            }
        }

        if self.sim.follow_module.enabled {
            self.sim
                .follow_module
                .begin_pass(&mut encoder, self.sim.physics_module.current);
            self.sim.follow_module.copy_buffer_to_buffer(&mut encoder);
        }

        self.gpu.queue.submit(Some(encoder.finish()));
        frame.present();

        #[cfg(feature = "capture")]
        if let Exists::Some(gfx) = &mut self.gfx {
            gfx.capture_module.get_frame(&self.gpu.device);
        }

        if self.sim.follow_module.enabled {
            if let Some(output) = self.sim.follow_module.get_data(&self.gpu.device) {
                self.sim.follow_module.info = output;

                if self.sim.follow_module.center_of_mass {
                    self.view_offset = -output.center_of_mass;
                    self.gfx.render_module.update_offset(
                        &self.gpu.queue,
                        self.view_offset.x,
                        self.view_offset.y,
                    );
                }

                if self.sim.follow_module.auto_zoom {
                    let size = (self.sim.follow_module.info.max_position
                        - self.sim.follow_module.info.min_position)
                        .abs();

                    self.view_zoom = size.length_recip().powf(0.75);
                    self.gfx
                        .render_module
                        .update_zoom(&self.gpu.queue, self.view_zoom);
                }
            }
        }

        self.framepace.end_frame(1.0 / self.framerate as f32);
    }
}
