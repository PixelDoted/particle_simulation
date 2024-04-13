use glam::Vec2;

#[derive(bytemuck::Zeroable, Clone, Copy)]
pub struct Particle {
    pub position: Vec2,
    pub velocity: Vec2,
    pub radius: f32,
    pub mass: f32,
}

unsafe impl bytemuck::Pod for Particle {}
