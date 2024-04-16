use std::borrow::Cow;

use glam::Vec2;

pub struct FollowModule {
    pub enabled: bool,
    pub view_center_of_mass: bool,
    pub view_scale: bool,

    pub center_of_mass: Vec2,
    pub size: Vec2,

    position_buffer: wgpu::Buffer,
    staging_buffer: wgpu::Buffer,

    bind_groups: [wgpu::BindGroup; 2],
    pipeline: wgpu::ComputePipeline,
}

impl FollowModule {
    pub fn new(device: &wgpu::Device, particle_buffers: &[wgpu::Buffer; 2]) -> Self {
        let follow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("follow.wgsl"))),
        });

        let position_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
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
            ],
        });

        let bind_group_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &particle_buffers[0],
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &position_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });
        let bind_group_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &particle_buffers[1],
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &position_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &follow_shader,
            entry_point: "main",
        });

        Self {
            enabled: false,
            view_center_of_mass: true,
            view_scale: true,

            center_of_mass: Vec2::ZERO,
            size: Vec2::ZERO,

            position_buffer,
            staging_buffer,

            bind_groups: [bind_group_a, bind_group_b],
            pipeline,
        }
    }

    pub fn begin_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
        particle_buffer_index: usize,
    ) {
        if !self.enabled {
            return;
        }

        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });

        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &self.bind_groups[particle_buffer_index], &[]);
        cpass.dispatch_workgroups(1, 1, 1);
    }

    pub fn copy_buffer_to_buffer(&self, encoder: &mut wgpu::CommandEncoder) {
        if !self.enabled {
            return;
        }

        encoder.copy_buffer_to_buffer(&self.position_buffer, 0, &self.staging_buffer, 0, 16);
    }

    pub fn get_data(&self, device: &wgpu::Device) -> Option<[Vec2; 2]> {
        self.enabled.then_some(())?;

        let slice = self.staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());

        device.poll(wgpu::Maintain::wait()).panic_on_timeout();
        if let Ok(Ok(())) = rx.recv() {
            let data = slice.get_mapped_range();
            let result: &[Vec2] = bytemuck::cast_slice(&data);
            let output: [Vec2; 2] = [result[0], result[1]];

            drop(data);
            self.staging_buffer.unmap();
            Some(output)
        } else {
            None
        }
    }
}
