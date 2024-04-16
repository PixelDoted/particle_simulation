//! Capture the rendered textures to generate a video file

use log::info;
use std::{io::Write, path::PathBuf};

use crate::render::RenderModule;

pub struct CaptureModule {
    pub enabled: bool,

    pub staging_buffer: wgpu::Buffer,
    pub texture: wgpu::Texture,

    buffer_file: std::fs::File,
}

impl CaptureModule {
    pub fn new(
        device: &wgpu::Device,
        texture_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let buffer_size =
            multiple_of(width, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) as u64 * height as u64 * 4;
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: texture_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        info!(
            "Capture texture info: {{ width: {}, height: {}, format: {:?} }}",
            width, height, texture_format
        );

        let path: PathBuf = "./frame_buffer.bin".into();
        if path.exists() {
            std::fs::remove_file(&path).expect("Failed to remove old `frame_buffer.bin`");
        }

        let file = std::fs::File::create(path).expect("Failed to create `frame_buffer.bin`");

        Self {
            enabled: false,

            staging_buffer,
            texture,
            buffer_file: file,
        }
    }

    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        texture_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) {
        let buffer_size =
            multiple_of(width, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) as u64 * height as u64 * 4;
        self.staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: texture_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        info!(
            "Capture texture info: {{ width: {}, height: {}, format: {:?} }}",
            width, height, texture_format
        );
    }

    pub fn begin_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        render_module: &RenderModule,
        particle_buffer: &wgpu::Buffer,
        num_particles: u32,
    ) {
        if !self.enabled {
            return;
        }

        let view = self
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        render_module.begin_pass(encoder, &view, particle_buffer, num_particles);
    }

    pub fn copy_texture_to_buffer(&self, encoder: &mut wgpu::CommandEncoder) {
        if !self.enabled {
            return;
        }

        let bytes_per_row =
            multiple_of(self.texture.width() * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);

        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::ImageCopyBuffer {
                buffer: &self.staging_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: None, //Some(width * height),
                },
            },
            wgpu::Extent3d {
                width: self.texture.width(),
                height: self.texture.height(),
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn get_frame(&mut self, device: &wgpu::Device) {
        if !self.enabled {
            return;
        }

        let slice = self.staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());

        device.poll(wgpu::Maintain::wait()).panic_on_timeout();
        if let Ok(Ok(())) = rx.recv() {
            let data = slice.get_mapped_range();
            let result: &[u8] = bytemuck::cast_slice(&data);

            let texture_width = self.texture.width();
            let bytes_per_row = multiple_of(texture_width, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * 4;
            for y in 0..self.texture.height() {
                let row = y * bytes_per_row;
                self.buffer_file
                    .write_all(&result[row as usize..row as usize + texture_width as usize * 4])
                    .unwrap();
            }

            self.buffer_file.flush().unwrap();

            drop(data);
            self.staging_buffer.unmap();
        } else {
            panic!("Failed to get view texture.");
        }
    }
}

fn multiple_of(mut value: u32, multiple: u32) -> u32 {
    let remainder = value % multiple;
    if remainder != 0 {
        value += multiple - remainder;
    }

    value
}
