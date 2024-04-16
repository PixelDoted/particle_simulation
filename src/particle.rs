use glam::Vec2;
use rand::{Rng, SeedableRng};

use crate::physics::PhysicsModule;

#[derive(bytemuck::Zeroable, Clone, Copy)]
pub struct Particle {
    pub position: Vec2,
    pub velocity: Vec2,
    pub radius: f32,
    pub mass: f32,
}

unsafe impl bytemuck::Pod for Particle {}

pub fn generate_particles(queue: &wgpu::Queue, physics_module: &PhysicsModule, num_particles: u64) {
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
                radius: 0.1, //rng.gen_range(0.01..=0.2f32),
                mass: 0.1,   //rng.gen_range(0.01..=0.2f32),
            };

            let i = c + p * (num_particles as u64 / 128);
            queue.write_buffer(
                physics_module.current_buffer(),
                i * 24,
                bytemuck::bytes_of(&particle),
            );
        }
    }
}
