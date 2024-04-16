use std::borrow::Cow;

use wgpu::util::DeviceExt;

use crate::particle::Particle;

pub struct PhysicsModule {
    pub particle_buffers: [wgpu::Buffer; 2],
    pub param_buffer: wgpu::Buffer,

    pub current: usize,

    bind_group_layout: wgpu::BindGroupLayout,
    pub bind_groups: [wgpu::BindGroup; 2],
    pub pipeline: wgpu::ComputePipeline,
}

impl PhysicsModule {
    pub fn new(device: &wgpu::Device, max_particles: usize, gravitational_constant: f32) -> Self {
        let physics_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("physics.wgsl"))),
        });

        // https://github.com/gfx-rs/wgpu/blob/trunk/examples/src/hello_compute/mod.rs
        // https://github.com/gfx-rs/wgpu/blob/trunk/examples/src/boids/mod.rs
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

        let (particle_buffers, bind_groups) =
            create_buffer_group(device, &bind_group_layout, &param_buffer, max_particles);

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &physics_shader,
            entry_point: "main",
        });

        Self {
            particle_buffers,
            param_buffer,

            current: 0,

            bind_group_layout,
            bind_groups,
            pipeline,
        }
    }

    pub fn resize_buffers(&mut self, device: &wgpu::Device, num_particles: usize) {
        let (particle_buffers, bind_groups) = create_buffer_group(
            device,
            &self.bind_group_layout,
            &self.param_buffer,
            num_particles,
        );

        self.particle_buffers = particle_buffers;
        self.bind_groups = bind_groups;
    }

    pub fn current_buffer(&self) -> &wgpu::Buffer {
        &self.particle_buffers[self.current]
    }

    pub fn begin_pass<'a>(
        &'a mut self,
        encoder: &'a mut wgpu::CommandEncoder,
        work_group_count: u32,
    ) -> wgpu::ComputePass<'a> {
        self.current = (self.current + 1) % 2;

        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });

        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &self.bind_groups[1 - self.current], &[]);
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

fn create_buffer_group(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    param_buffer: &wgpu::Buffer,
    num_particles: usize,
) -> ([wgpu::Buffer; 2], [wgpu::BindGroup; 2]) {
    let pba = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (std::mem::size_of::<Particle>() * num_particles) as u64,
        usage: wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let pbb = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (std::mem::size_of::<Particle>() * num_particles) as u64,
        usage: wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bga = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: pba.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: pbb.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: param_buffer.as_entire_binding(),
            },
        ],
    });
    let bgb = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: pbb.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: pba.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: param_buffer.as_entire_binding(),
            },
        ],
    });

    ([pba, pbb], [bga, bgb])
}
