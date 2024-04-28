use std::sync::Arc;

use winit::window::Window;

pub struct GpuContext<'a> {
    pub surface: wgpu::Surface<'a>,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
}

impl<'a> GpuContext<'a> {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let window_size = window.inner_size();

        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("Failed to find an appropriate adapter");
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .expect("Failed to create Device");

        let config = surface
            .get_default_config(&adapter, window_size.width, window_size.height)
            .unwrap();
        surface.configure(&device, &config);

        Ok(Self {
            surface,
            adapter,
            device,
            queue,
            config,
        })
    }

    pub fn reconfigure_surface(&self) {
        self.surface.configure(&self.device, &self.config);
    }

    pub fn surface_capabilities(&self) -> wgpu::SurfaceCapabilities {
        self.surface.get_capabilities(&self.adapter)
        // let swapchain_format = swapchain_capabilities.formats[0];
    }
}
