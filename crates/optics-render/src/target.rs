//! Render targets and texture buffer management.

use crate::context::GpuContext;

/// HDR Accumulation render target holding raw high-dynamic-range radiance samples.
#[derive(Debug)]
pub struct RenderTarget {
    /// 2D texture allocation.
    pub texture: wgpu::Texture,
    /// Default texture view.
    pub view: wgpu::TextureView,
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Pixel format (typically [`wgpu::TextureFormat::Rgba16Float`] or `Rgba32Float`).
    pub format: wgpu::TextureFormat,
}

impl RenderTarget {
    /// Creates a new HDR accumulation target of the specified dimensions.
    #[must_use]
    pub fn new(ctx: &GpuContext, width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let format = wgpu::TextureFormat::Rgba16Float;

        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HDR Accumulation Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            view,
            width,
            height,
            format,
        }
    }
}

/// Tone-mapped display target for presentation or window surface blitting.
#[derive(Debug)]
pub struct ToneMapTarget {
    /// 2D display texture allocation.
    pub texture: wgpu::Texture,
    /// Default texture view.
    pub view: wgpu::TextureView,
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Display format (e.g. [`wgpu::TextureFormat::Rgba8Unorm`]).
    pub format: wgpu::TextureFormat,
}

impl ToneMapTarget {
    /// Creates a new LDR display target of the specified dimensions.
    #[must_use]
    pub fn new(ctx: &GpuContext, width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        let width = width.max(1);
        let height = height.max(1);

        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ToneMapped Display Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            view,
            width,
            height,
            format,
        }
    }
}

/// Helper function to resize accumulation and tone-mapped targets simultaneously.
#[must_use]
pub fn resize_render_targets(
    ctx: &GpuContext,
    width: u32,
    height: u32,
    display_format: wgpu::TextureFormat,
) -> (RenderTarget, ToneMapTarget) {
    let hdr = RenderTarget::new(ctx, width, height);
    let ldr = ToneMapTarget::new(ctx, width, height, display_format);
    (hdr, ldr)
}
