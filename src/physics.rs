use std::borrow::Cow;

use wgpu::util::DeviceExt;

use crate::types::Particle;

pub struct PhysicsModule {
    pub particle_buffer_collision: wgpu::Buffer,
    pub particle_buffer_gravity: wgpu::Buffer,

    pub param_buffer: wgpu::Buffer,

    pub particle_count: u32,

    pub bind_group_collision: wgpu::BindGroup,
    pub bind_group_gravity: wgpu::BindGroup,

    pub collision_pipeline: wgpu::ComputePipeline,
    pub gravity_pipeline: wgpu::ComputePipeline,
}

impl PhysicsModule {
    pub fn new(device: &wgpu::Device, max_particles: usize, gravitational_constant: f32) -> Self {
        let gravity_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("gravity.wgsl"))),
        });
        let collision_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("collision.wgsl"))),
        });

        // https://github.com/gfx-rs/wgpu/blob/trunk/examples/src/hello_compute/mod.rs
        // https://github.com/gfx-rs/wgpu/blob/trunk/examples/src/boids/mod.rs
        let particle_buffer_collision = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (std::mem::size_of::<Particle>() * max_particles) as u64,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let particle_buffer_gravity = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (std::mem::size_of::<Particle>() * max_particles) as u64,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let param_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Physics Parameter Buffer"),
            contents: bytemuck::cast_slice(&[1.0f32, gravitational_constant]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group_collision = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: particle_buffer_collision.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: particle_buffer_gravity.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: param_buffer.as_entire_binding(),
                },
            ],
        });
        let bind_group_gravity = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: particle_buffer_gravity.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: particle_buffer_collision.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: param_buffer.as_entire_binding(),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let gravity_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &gravity_module,
            entry_point: "main",
        });
        let collision_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &collision_module,
            entry_point: "main",
        });

        Self {
            particle_buffer_collision,
            particle_buffer_gravity,
            param_buffer,

            particle_count: max_particles as u32,

            bind_group_collision,
            bind_group_gravity,
            collision_pipeline,
            gravity_pipeline,
        }
    }

    pub fn begin_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
        work_group_count: u32,
    ) -> wgpu::ComputePass<'a> {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });

        cpass.set_pipeline(&self.collision_pipeline);
        cpass.set_bind_group(0, &self.bind_group_collision, &[]);
        cpass.dispatch_workgroups(work_group_count, 1, 1);

        cpass.set_pipeline(&self.gravity_pipeline);
        cpass.set_bind_group(0, &self.bind_group_gravity, &[]);
        cpass.dispatch_workgroups(work_group_count, 1, 1);

        cpass
    }

    pub fn update_delta_time(&self, queue: &wgpu::Queue, dt: f32) {
        queue.write_buffer(&self.param_buffer, 0, bytemuck::bytes_of(&dt));
    }

    pub fn update_gravitational_constant(&self, queue: &wgpu::Queue, g: f32) {
        queue.write_buffer(&self.param_buffer, 4, bytemuck::bytes_of(&g));
    }
}
