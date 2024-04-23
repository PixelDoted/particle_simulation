use std::borrow::Cow;

use wgpu::{util::DeviceExt, BindGroupLayoutEntry};

pub struct RenderModule {
    pub screen_size_buffer: wgpu::Buffer,
    pub viewport_buffer: wgpu::Buffer,
    vertices_buffer: wgpu::Buffer,

    pub bind_group: wgpu::BindGroup,
    pub pipeline: wgpu::RenderPipeline,
}

impl RenderModule {
    pub fn new(device: &wgpu::Device, swapchain_format: wgpu::TextureFormat) -> Self {
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("render.wgsl"))),
        });

        let screen_size_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 2 * 4,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let viewport_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 4 * 4,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let vertices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&[-1.0f32, -1.0, 1.0, -1.0, 0.0, 1.0]),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: screen_size_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: viewport_buffer.as_entire_binding(),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: "vertex",
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: 6 * 4,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32, 3 => Float32],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: 2 * 4,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![4 => Float32x2],
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: "fragment",
                targets: &[Some(swapchain_format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self {
            screen_size_buffer,
            viewport_buffer,
            vertices_buffer,

            bind_group,
            pipeline,
        }
    }

    pub fn begin_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
        view: &'a wgpu::TextureView,
        particle_buffer: &'a wgpu::Buffer,
        num_particles: u32,
    ) -> wgpu::RenderPass<'a> {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_vertex_buffer(0, particle_buffer.slice(..));
        rpass.set_vertex_buffer(1, self.vertices_buffer.slice(..));
        rpass.draw(0..3, 0..num_particles);

        rpass
    }

    pub fn update_size(&self, queue: &wgpu::Queue, width: u32, height: u32) {
        queue.write_buffer(
            &self.screen_size_buffer,
            0,
            bytemuck::bytes_of(&[width as f32, height as f32]),
        );
    }

    pub fn update_offset(&self, queue: &wgpu::Queue, x: f32, y: f32) {
        queue.write_buffer(&self.viewport_buffer, 0, bytemuck::bytes_of(&[x, y]));
    }

    pub fn update_zoom(&self, queue: &wgpu::Queue, zoom: f32) {
        queue.write_buffer(&self.viewport_buffer, 8, bytemuck::bytes_of(&[zoom]));
    }

    pub fn update_all(
        &self,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        x: f32,
        y: f32,
        zoom: f32,
    ) {
        queue.write_buffer(
            &self.screen_size_buffer,
            0,
            bytemuck::bytes_of(&[width as f32, height as f32]),
        );
        queue.write_buffer(
            &self.viewport_buffer,
            0,
            bytemuck::bytes_of(&[x, y, zoom, 0f32]),
        );
    }
}
