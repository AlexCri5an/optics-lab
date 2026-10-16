//! WebGPU / wgpu hardware context abstraction.

use crate::error::RenderError;

/// Active GPU hardware context holding instance, adapter, device, and command queue.
#[derive(Debug)]
pub struct GpuContext {
    /// wgpu instance handle.
    pub instance: wgpu::Instance,
    /// Selected physical graphics/compute adapter.
    pub adapter: wgpu::Adapter,
    /// Logical GPU device handle for resource allocation.
    pub device: wgpu::Device,
    /// Default submission command queue.
    pub queue: wgpu::Queue,
}

impl GpuContext {
    /// Creates a GPU context from existing wgpu handles.
    #[must_use]
    pub fn from_parts(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Self {
        Self {
            instance,
            adapter,
            device,
            queue,
        }
    }

    /// Creates a headless GPU context suitable for offline rendering, tests, and CLI execution.
    ///
    /// # Errors
    /// Returns [`RenderError::AdapterRequestFailed`] if no GPU adapter matches, or
    /// [`RenderError::DeviceRequestFailed`] if device acquisition fails.
    pub async fn create_headless() -> Result<Self, RenderError> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|_| RenderError::AdapterRequestFailed)?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("OpticsLab Headless Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: Default::default(),
            })
            .await
            .map_err(|e| RenderError::DeviceRequestFailed(e.to_string()))?;

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }
}
