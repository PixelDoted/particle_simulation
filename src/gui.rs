use glam::Vec2;

pub struct EguiIntegration {
    pub ctx: egui::Context,
    raw_input: egui::RawInput,
    modifiers: egui::Modifiers,

    renderer: egui_wgpu::Renderer,
    clipped_shapes: Vec<egui::ClippedPrimitive>,
    textures_delta: egui::TexturesDelta,
}

impl EguiIntegration {
    pub fn new(device: &wgpu::Device, swapchain_format: wgpu::TextureFormat) -> Self {
        let renderer = egui_wgpu::Renderer::new(device, swapchain_format, None, 1);

        Self {
            ctx: egui::Context::default(),
            raw_input: egui::RawInput::default(),
            modifiers: Default::default(),

            renderer,
            clipped_shapes: Vec::new(),
            textures_delta: egui::TexturesDelta::default(),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.raw_input.screen_rect = Some(egui::Rect::from_min_size(
            Default::default(),
            egui::Vec2::new(width as f32, height as f32),
        ));
    }

    pub fn run<F: FnOnce(&egui::Context)>(&mut self, run_ui: F) {
        let raw_input = std::mem::take(&mut self.raw_input);
        self.ctx.begin_frame(raw_input);
        run_ui(&self.ctx);

        let output = self.ctx.end_frame();
        self.clipped_shapes = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        self.textures_delta = output.textures_delta;
    }

    pub fn pre_render<'a>(
        &mut self,
        device: &'a wgpu::Device,
        queue: &'a wgpu::Queue,
        encoder: &'a mut wgpu::CommandEncoder,
        delta_time: f32,
    ) {
        self.raw_input.predicted_dt = delta_time;

        let screen_rect = self.ctx.screen_rect();
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [screen_rect.width() as u32, screen_rect.height() as u32],
            pixels_per_point: self.ctx.pixels_per_point(),
        };

        self.renderer.update_buffers(
            device,
            queue,
            encoder,
            &self.clipped_shapes,
            &screen_descriptor,
        );

        for (id, delta) in &self.textures_delta.set {
            self.renderer.update_texture(device, queue, *id, delta);
        }

        for id in &self.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }

    pub fn render<'a>(&'a mut self, rpass: &mut wgpu::RenderPass<'a>) {
        let screen_rect = self.ctx.screen_rect();
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [screen_rect.width() as u32, screen_rect.height() as u32],
            pixels_per_point: self.ctx.pixels_per_point(),
        };

        self.renderer
            .render(rpass, &self.clipped_shapes, &screen_descriptor);
    }

    pub fn modifiers_event(&mut self, event: winit::event::Modifiers) {
        let state = event.state();
        self.modifiers.alt = state.alt_key();
        self.modifiers.ctrl = state.control_key();
        self.modifiers.shift = state.shift_key();
        self.modifiers.mac_cmd = state.super_key();
        self.modifiers.command = state.control_key() | state.super_key();
    }

    pub fn key_event(&mut self, event: winit::event::KeyEvent) -> Option<()> {
        let pressed = matches!(event.state, winit::event::ElementState::Pressed);
        let repeat = event.repeat;
        let key = match event.logical_key {
            winit::keyboard::Key::Named(key) => named_key_to_egui_key(key)?,
            winit::keyboard::Key::Character(char) => {
                if pressed {
                    self.raw_input
                        .events
                        .push(egui::Event::Text(char.to_string()));
                }

                return None;
            }
            winit::keyboard::Key::Unidentified(_) => return None,
            winit::keyboard::Key::Dead(_) => return None,
        };
        let physical_key = match event.physical_key {
            winit::keyboard::PhysicalKey::Code(code) => keycode_to_egui_key(code),
            winit::keyboard::PhysicalKey::Unidentified(_) => None,
        };

        self.raw_input.events.push(egui::Event::Key {
            key,
            physical_key,
            pressed,
            repeat,
            modifiers: self.modifiers,
        });
        None
    }

    pub fn mouse_event(
        &mut self,
        position: Vec2,
        state: winit::event::ElementState,
        button: winit::event::MouseButton,
    ) {
        let pressed = matches!(state, winit::event::ElementState::Pressed);
        let button = match button {
            winit::event::MouseButton::Left => egui::PointerButton::Primary,
            winit::event::MouseButton::Right => egui::PointerButton::Secondary,
            winit::event::MouseButton::Middle => egui::PointerButton::Middle,
            winit::event::MouseButton::Back => egui::PointerButton::Extra1,
            winit::event::MouseButton::Forward => egui::PointerButton::Extra2,
            winit::event::MouseButton::Other(_) => return,
        };

        self.raw_input.events.push(egui::Event::PointerButton {
            pos: egui::Pos2::new(position.x, position.y),
            button,
            pressed,
            modifiers: self.modifiers,
        });
    }

    pub fn mouse_motion(&mut self, position: Vec2) {
        self.raw_input
            .events
            .push(egui::Event::PointerMoved(egui::Pos2::new(
                position.x, position.y,
            )));
    }
}

fn keycode_to_egui_key(key: winit::keyboard::KeyCode) -> Option<egui::Key> {
    use winit::keyboard::KeyCode;
    Some(match key {
        KeyCode::Backslash => egui::Key::Backslash,
        KeyCode::BracketLeft => egui::Key::OpenBracket,
        KeyCode::BracketRight => egui::Key::CloseBracket,
        KeyCode::Comma | KeyCode::NumpadComma => egui::Key::Comma,
        KeyCode::Digit0 | KeyCode::Numpad0 => egui::Key::Num0,
        KeyCode::Digit1 | KeyCode::Numpad1 => egui::Key::Num1,
        KeyCode::Digit2 | KeyCode::Numpad2 => egui::Key::Num2,
        KeyCode::Digit3 | KeyCode::Numpad3 => egui::Key::Num3,
        KeyCode::Digit4 | KeyCode::Numpad4 => egui::Key::Num4,
        KeyCode::Digit5 | KeyCode::Numpad5 => egui::Key::Num5,
        KeyCode::Digit6 | KeyCode::Numpad6 => egui::Key::Num6,
        KeyCode::Digit7 | KeyCode::Numpad7 => egui::Key::Num7,
        KeyCode::Digit8 | KeyCode::Numpad8 => egui::Key::Num8,
        KeyCode::Digit9 | KeyCode::Numpad9 => egui::Key::Num9,
        KeyCode::Equal => egui::Key::Equals,
        KeyCode::KeyA => egui::Key::A,
        KeyCode::KeyB => egui::Key::B,
        KeyCode::KeyC => egui::Key::C,
        KeyCode::KeyD => egui::Key::D,
        KeyCode::KeyE => egui::Key::E,
        KeyCode::KeyF => egui::Key::F,
        KeyCode::KeyG => egui::Key::G,
        KeyCode::KeyH => egui::Key::H,
        KeyCode::KeyI => egui::Key::I,
        KeyCode::KeyJ => egui::Key::J,
        KeyCode::KeyK => egui::Key::K,
        KeyCode::KeyL => egui::Key::L,
        KeyCode::KeyM => egui::Key::M,
        KeyCode::KeyN => egui::Key::N,
        KeyCode::KeyO => egui::Key::O,
        KeyCode::KeyP => egui::Key::P,
        KeyCode::KeyQ => egui::Key::Q,
        KeyCode::KeyR => egui::Key::R,
        KeyCode::KeyS => egui::Key::S,
        KeyCode::KeyT => egui::Key::T,
        KeyCode::KeyU => egui::Key::U,
        KeyCode::KeyV => egui::Key::V,
        KeyCode::KeyW => egui::Key::W,
        KeyCode::KeyX => egui::Key::X,
        KeyCode::KeyY => egui::Key::Y,
        KeyCode::KeyZ => egui::Key::Z,
        KeyCode::Minus | KeyCode::NumpadSubtract => egui::Key::Minus,
        KeyCode::Period | KeyCode::NumpadDecimal => egui::Key::Period,
        KeyCode::Semicolon => egui::Key::Semicolon,
        KeyCode::Slash | KeyCode::NumpadDivide => egui::Key::Slash,
        KeyCode::Backspace | KeyCode::NumpadBackspace => egui::Key::Backspace,
        KeyCode::Enter | KeyCode::NumpadEnter => egui::Key::Enter,
        KeyCode::Space => egui::Key::Space,
        KeyCode::Tab => egui::Key::Tab,
        KeyCode::Delete => egui::Key::Delete,
        KeyCode::End => egui::Key::End,
        KeyCode::Home => egui::Key::Home,
        KeyCode::Insert => egui::Key::Insert,
        KeyCode::PageDown => egui::Key::PageDown,
        KeyCode::PageUp => egui::Key::PageUp,
        KeyCode::ArrowDown => egui::Key::ArrowDown,
        KeyCode::ArrowLeft => egui::Key::ArrowLeft,
        KeyCode::ArrowRight => egui::Key::ArrowRight,
        KeyCode::ArrowUp => egui::Key::ArrowUp,
        KeyCode::NumpadAdd => egui::Key::Plus,
        KeyCode::NumpadEqual => egui::Key::Equals,
        KeyCode::Escape => egui::Key::Escape,
        _ => return None,
    })
}

fn named_key_to_egui_key(key: winit::keyboard::NamedKey) -> Option<egui::Key> {
    use winit::keyboard::NamedKey;
    Some(match key {
        NamedKey::Backspace => egui::Key::Backspace,
        NamedKey::Enter => egui::Key::Enter,
        NamedKey::Space => egui::Key::Space,
        NamedKey::Tab => egui::Key::Tab,
        NamedKey::Delete => egui::Key::Delete,
        NamedKey::End => egui::Key::End,
        NamedKey::Home => egui::Key::Home,
        NamedKey::Insert => egui::Key::Insert,
        NamedKey::PageDown => egui::Key::PageDown,
        NamedKey::PageUp => egui::Key::PageUp,
        NamedKey::ArrowDown => egui::Key::ArrowDown,
        NamedKey::ArrowLeft => egui::Key::ArrowLeft,
        NamedKey::ArrowRight => egui::Key::ArrowRight,
        NamedKey::ArrowUp => egui::Key::ArrowUp,
        NamedKey::Escape => egui::Key::Escape,
        _ => return None,
    })
}
