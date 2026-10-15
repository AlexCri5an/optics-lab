//! Path tracing orchestration pipeline, compute passes, and tonemapping execution.

use bytemuck::{Pod, Zeroable};
use optics_bvh::BvhScene;
use wgpu::util::DeviceExt;

use crate::bvh_upload::BvhBuffer;
use crate::camera::ThinLensCamera;
use crate::context::GpuContext;
use crate::error::RenderError;
use crate::shaders::PATH_TRACER_WGSL;
use crate::target::{RenderTarget, ToneMapTarget};

/// Diagnostic rendering modes selectable in the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum RenderMode {
    /// Full physical path tracing with refraction and spectral accumulation.
    #[default]
    Standard = 0,
    /// Traversal cost heatmap visualization.
    BvhHeatmap = 1,
    /// Geometry surface normal orientation.
    SurfaceNormals = 2,
}

/// Dynamic parameters passed to the path tracing compute shader.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct RenderParams {
    /// Monotonically increasing frame index.
    pub frame_index: u32,
    /// Current progressive sample iteration count.
    pub sample_count: u32,
    /// Maximum ray bounce depth.
    pub max_bounces: u32,
    /// Active render diagnostic mode ([`RenderMode`]).
    pub render_mode: u32,
}

/// Orchestrates GPU path tracing passes, accumulation, and presentation.
#[derive(Debug)]
pub struct PathTracingRenderer {
    /// Active GPU hardware context.
    pub context: GpuContext,
    /// Uploaded BVH hierarchy and triangle primitive buffers.
    pub bvh_buffer: BvhBuffer,
    /// Path tracing compute pipeline.
    pub compute_pipeline: wgpu::ComputePipeline,
    /// Compute pass bind group.
    pub compute_bind_group: wgpu::BindGroup,
    /// Camera uniform buffer.
    pub camera_buffer: wgpu::Buffer,
    /// Render parameters uniform buffer.
    pub params_buffer: wgpu::Buffer,
    /// HDR Accumulation render target.
    pub accumulation_target: RenderTarget,
    /// Tone-mapped display target.
    pub display_target: ToneMapTarget,
    /// Number of accumulated samples.
    pub sample_count: u64,
    /// Monotonic frame counter.
    pub frame_counter: u64,
    /// Active rendering mode.
    pub render_mode: RenderMode,
    /// Maximum ray bounces per path.
    pub max_bounces: u32,
}

impl PathTracingRenderer {
    /// Creates a new path tracing renderer for the given scene and viewport dimensions.
    ///
    /// # Errors
    /// Returns [`RenderError`] if pipeline creation or buffer allocation fails.
    pub fn new(
        context: GpuContext,
        scene: &BvhScene,
        camera: &ThinLensCamera,
        width: u32,
        height: u32,
        display_format: wgpu::TextureFormat,
    ) -> Result<Self, RenderError> {
        let bvh_buffer = BvhBuffer::upload(&context, scene);
        let accumulation_target = RenderTarget::new(&context, width, height);
        let display_target = ToneMapTarget::new(&context, width, height, display_format);

        let aspect_ratio = width as f64 / height.max(1) as f64;
        let camera_uniform = camera.to_uniform(aspect_ratio);

        let camera_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Camera Uniform Buffer"),
                contents: bytemuck::bytes_of(&camera_uniform),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let initial_params = RenderParams {
            frame_index: 0,
            sample_count: 0,
            max_bounces: 6,
            render_mode: RenderMode::Standard as u32,
        };

        let params_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Render Params Buffer"),
                contents: bytemuck::bytes_of(&initial_params),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let compute_shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Path Tracer Compute Shader"),
                source: wgpu::ShaderSource::Wgsl(PATH_TRACER_WGSL.into()),
            });

        let compute_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Compute Bind Group Layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 4,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::StorageTexture {
                                access: wgpu::StorageTextureAccess::WriteOnly,
                                format: wgpu::TextureFormat::Rgba16Float,
                                view_dimension: wgpu::TextureViewDimension::D2,
                            },
                            count: None,
                        },
                    ],
                });

        let compute_bind_group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Compute Bind Group"),
                layout: &compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: camera_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: bvh_buffer.node_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: bvh_buffer.triangle_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&accumulation_target.view),
                    },
                ],
            });

        let compute_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Compute Pipeline Layout"),
                    bind_group_layouts: &[&compute_bind_group_layout],
                    push_constant_ranges: &[],
                });

        let compute_pipeline =
            context
                .device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("Path Tracer Compute Pipeline"),
                    layout: Some(&compute_pipeline_layout),
                    module: &compute_shader,
                    entry_point: Some("main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    cache: None,
                });

        Ok(Self {
            context,
            bvh_buffer,
            compute_pipeline,
            compute_bind_group,
            camera_buffer,
            params_buffer,
            accumulation_target,
            display_target,
            sample_count: 0,
            frame_counter: 0,
            render_mode: RenderMode::Standard,
            max_bounces: 6,
        })
    }

    /// Resets the progressive sample accumulation count.
    pub fn reset_accumulation(&mut self) {
        self.sample_count = 0;
    }

    /// Returns the number of accumulated samples.
    #[must_use]
    pub fn get_sample_count(&self) -> u64 {
        self.sample_count
    }

    /// Executes one frame of compute path tracing.
    pub fn render_frame(&mut self) {
        let params = RenderParams {
            frame_index: self.frame_counter as u32,
            sample_count: self.sample_count as u32,
            max_bounces: self.max_bounces,
            render_mode: self.render_mode as u32,
        };

        self.context
            .queue
            .write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Path Tracer Command Encoder"),
                });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Path Tracing Compute Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);

            let workgroups_x = self.accumulation_target.width.div_ceil(8);
            let workgroups_y = self.accumulation_target.height.div_ceil(8);
            compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }

        self.context.queue.submit(Some(encoder.finish()));

        self.sample_count += 1;
        self.frame_counter += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_render_params_alignment() -> TestResult {
        assert_eq!(
            std::mem::align_of::<RenderParams>(),
            16,
            "RenderParams must be 16-byte aligned"
        );
        assert_eq!(
            std::mem::size_of::<RenderParams>(),
            16,
            "RenderParams must be 16 bytes"
        );
        Ok(())
    }

    #[test]
    fn test_headless_render_frame() -> TestResult {
        // Run async headless test using pollster or futures block_on
        let fut = async {
            let ctx = match GpuContext::create_headless().await {
                Ok(c) => c,
                Err(RenderError::AdapterRequestFailed) => {
                    // No GPU adapter in CI environment, skip test gracefully
                    return Ok(());
                }
                Err(e) => return Err(Box::new(e) as Box<dyn std::error::Error>),
            };

            let tri = optics_bvh::Triangle::new(
                optics_core::Vec3::new(-1.0, -1.0, 5.0),
                optics_core::Vec3::new(1.0, -1.0, 5.0),
                optics_core::Vec3::new(0.0, 1.0, 5.0),
                0,
            );
            let aabb = tri.aabb();
            let inst = optics_bvh::BlasInstance::new(0, &aabb, nalgebra::Matrix4::identity());
            let scene = BvhScene::build(vec![vec![tri]], vec![inst]);

            let camera = ThinLensCamera::new(
                optics_core::Vec3::new(0.0, 0.0, 0.0),
                optics_core::Vec3::new(0.0, 0.0, 5.0),
                optics_core::Vec3::new(0.0, 1.0, 0.0),
                60.0,
                0.0,
                5.0,
            );

            let mut renderer = PathTracingRenderer::new(
                ctx,
                &scene,
                &camera,
                64,
                64,
                wgpu::TextureFormat::Rgba8Unorm,
            )?;

            assert_eq!(renderer.get_sample_count(), 0);
            renderer.render_frame();
            assert_eq!(renderer.get_sample_count(), 1);

            Ok(())
        };

        block_on(fut)
    }

    fn block_on<F: std::future::Future>(fut: F) -> F::Output {
        let mut fut = std::pin::pin!(fut);
        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);
        loop {
            match fut.as_mut().poll(&mut cx) {
                std::task::Poll::Ready(res) => return res,
                std::task::Poll::Pending => std::thread::yield_now(),
            }
        }
    }
}
