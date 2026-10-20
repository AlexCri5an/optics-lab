//! Path tracing orchestration pipeline, compute passes, and tonemapping execution.

use bytemuck::{Pod, Zeroable};
use optics_bvh::BvhScene;
use wgpu::util::DeviceExt;

use crate::bvh_upload::BvhBuffer;
use crate::camera::ThinLensCamera;
use crate::context::GpuContext;
use crate::error::RenderError;
use crate::material::{GpuMaterial, pack_materials_to_gpu_buffer};
use crate::shaders::{PATH_TRACER_WGSL, TONEMAPPING_WGSL};
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
    /// Sellmeier B1 coefficient.
    pub b1: f32,
    /// Sellmeier C1 coefficient.
    pub c1: f32,
    /// Sellmeier B2 coefficient.
    pub b2: f32,
    /// Sellmeier C2 coefficient.
    pub c2: f32,
    /// Sellmeier B3 coefficient.
    pub b3: f32,
    /// Sellmeier C3 coefficient.
    pub c3: f32,
    /// Uniform alignment padding.
    pub _pad: [f32; 2],
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
    /// Compute pass bind group layout.
    pub compute_bind_group_layout: wgpu::BindGroupLayout,
    /// Camera uniform buffer.
    pub camera_buffer: wgpu::Buffer,
    /// Render parameters uniform buffer.
    pub params_buffer: wgpu::Buffer,
    /// Progressive accumulation storage buffer.
    pub accum_buffer: wgpu::Buffer,
    /// HDR Accumulation render target.
    pub accumulation_target: RenderTarget,
    /// Tone-mapped display target.
    pub display_target: ToneMapTarget,
    /// Tonemapping render pipeline.
    pub tonemap_pipeline: wgpu::RenderPipeline,
    /// Tonemapping bind group.
    pub tonemap_bind_group: wgpu::BindGroup,
    /// Tonemapping bind group layout.
    pub tonemap_bind_group_layout: wgpu::BindGroupLayout,
    /// Configured display target texture format.
    pub display_format: wgpu::TextureFormat,
    /// Number of accumulated samples.
    pub sample_count: u64,
    /// Monotonic frame counter.
    pub frame_counter: u64,
    /// Active rendering mode.
    pub render_mode: RenderMode,
    /// Maximum ray bounces per path.
    pub max_bounces: u32,
    /// Active Sellmeier dispersion B coefficients [B1, B2, B3].
    pub sellmeier_b: [f32; 3],
    /// Active Sellmeier dispersion C coefficients [C1, C2, C3].
    pub sellmeier_c: [f32; 3],
    /// Uploaded material storage buffer.
    pub material_buffer: wgpu::Buffer,
    /// Active material definitions.
    pub materials: Vec<GpuMaterial>,
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

        // Default Schott N-BK7 Sellmeier coefficients
        let default_b = [1.039_612_2_f32, 0.231_792_35_f32, 1.010_469_4_f32];
        let default_c = [0.006_000_698_5_f32, 0.020_017_914_f32, 103.560_65_f32];

        let initial_params = RenderParams {
            frame_index: 0,
            sample_count: 0,
            max_bounces: 6,
            render_mode: RenderMode::Standard as u32,
            b1: default_b[0],
            c1: default_c[0],
            b2: default_b[1],
            c2: default_c[1],
            b3: default_b[2],
            c3: default_c[2],
            _pad: [0.0; 2],
        };

        let params_buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Render Params Buffer"),
                contents: bytemuck::bytes_of(&initial_params),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let default_materials = vec![
            GpuMaterial::n_bk7(),
            GpuMaterial::glossy(0.05, 0.04, [0.02, 0.022, 0.028]),
            GpuMaterial::diffuse([0.8, 0.8, 0.8]),
            GpuMaterial::sk16(),
            GpuMaterial::f2(),
            GpuMaterial::sf11(),
            GpuMaterial::fused_silica(),
        ];
        let material_bytes = pack_materials_to_gpu_buffer(&default_materials);
        let material_buffer =
            context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Material Storage Buffer"),
                    contents: &material_bytes,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                });

        let accum_size = (width.max(1) as u64) * (height.max(1) as u64) * 16;
        let accum_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HDR Accumulation Storage Buffer"),
            size: accum_size.max(64),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
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
                        wgpu::BindGroupLayoutEntry {
                            binding: 5,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 6,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
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
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: accum_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: material_buffer.as_entire_binding(),
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

        // Tonemapping presentation pipeline
        let tonemap_shader = context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Tonemapping Shader Module"),
                source: wgpu::ShaderSource::Wgsl(TONEMAPPING_WGSL.into()),
            });

        let tonemap_bind_group_layout =
            context
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Tonemapping Bind Group Layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    }],
                });

        let tonemap_bind_group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Tonemapping Bind Group"),
                layout: &tonemap_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&accumulation_target.view),
                }],
            });

        let tonemap_pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Tonemapping Pipeline Layout"),
                    bind_group_layouts: &[&tonemap_bind_group_layout],
                    push_constant_ranges: &[],
                });

        let tonemap_pipeline =
            context
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Tonemapping Render Pipeline"),
                    layout: Some(&tonemap_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &tonemap_shader,
                        entry_point: Some("vs_main"),
                        buffers: &[],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &tonemap_shader,
                        entry_point: Some("fs_main"),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: display_format,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });

        Ok(Self {
            context,
            bvh_buffer,
            compute_pipeline,
            compute_bind_group,
            compute_bind_group_layout,
            camera_buffer,
            params_buffer,
            accum_buffer,
            accumulation_target,
            display_target,
            tonemap_pipeline,
            tonemap_bind_group,
            tonemap_bind_group_layout,
            display_format,
            sample_count: 0,
            frame_counter: 0,
            render_mode: RenderMode::Standard,
            max_bounces: 6,
            sellmeier_b: default_b,
            sellmeier_c: default_c,
            material_buffer,
            materials: default_materials,
        })
    }

    /// Updates the GPU material library buffer and resets sample accumulation.
    pub fn update_materials(&mut self, materials: &[GpuMaterial]) {
        self.materials = materials.to_vec();
        let bytes = pack_materials_to_gpu_buffer(&self.materials);
        self.material_buffer =
            self.context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Material Storage Buffer"),
                    contents: &bytes,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                });
        self.rebuild_compute_bind_group();
        self.reset_accumulation();
    }

    /// Updates the Sellmeier dispersion coefficients used for real glass spectral path tracing.
    pub fn set_material_sellmeier(&mut self, b: [f32; 3], c: [f32; 3]) {
        if self.sellmeier_b != b || self.sellmeier_c != c {
            self.sellmeier_b = b;
            self.sellmeier_c = c;
            self.reset_accumulation();
        }
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

    /// Updates the camera parameters and resets sample accumulation.
    pub fn update_camera(&mut self, camera: &ThinLensCamera) {
        let aspect =
            self.accumulation_target.width as f64 / self.accumulation_target.height.max(1) as f64;
        let uniform = camera.to_uniform(aspect);
        self.context
            .queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
        self.reset_accumulation();
    }

    /// Updates the scene geometry and rebuilds the BVH buffer.
    pub fn update_scene(&mut self, scene: &BvhScene) {
        self.bvh_buffer = BvhBuffer::upload(&self.context, scene);
        self.rebuild_compute_bind_group();
        self.reset_accumulation();
    }

    /// Resizes the accumulation and display targets.
    /// Returns `true` if targets were reallocated, `false` if dimensions were unchanged.
    pub fn resize(&mut self, width: u32, height: u32) -> bool {
        let width = width.max(1);
        let height = height.max(1);
        if self.accumulation_target.width == width && self.accumulation_target.height == height {
            return false;
        }

        self.accumulation_target = RenderTarget::new(&self.context, width, height);
        self.display_target = ToneMapTarget::new(&self.context, width, height, self.display_format);

        let accum_size = (width as u64) * (height as u64) * 16;
        self.accum_buffer = self.context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("HDR Accumulation Storage Buffer"),
            size: accum_size.max(64),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.rebuild_compute_bind_group();
        self.rebuild_tonemap_bind_group();
        self.reset_accumulation();
        true
    }

    fn rebuild_compute_bind_group(&mut self) {
        self.compute_bind_group =
            self.context
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Compute Bind Group"),
                    layout: &self.compute_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.camera_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: self.params_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: self.bvh_buffer.node_buf.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: self.bvh_buffer.triangle_buf.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::TextureView(
                                &self.accumulation_target.view,
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: self.accum_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: self.material_buffer.as_entire_binding(),
                        },
                    ],
                });
    }

    fn rebuild_tonemap_bind_group(&mut self) {
        self.tonemap_bind_group =
            self.context
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Tonemapping Bind Group"),
                    layout: &self.tonemap_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &self.accumulation_target.view,
                        ),
                    }],
                });
    }

    /// Executes one frame of compute path tracing and tonemapping.
    pub fn render_frame(&mut self) {
        let params = RenderParams {
            frame_index: self.frame_counter as u32,
            sample_count: self.sample_count as u32,
            max_bounces: self.max_bounces,
            render_mode: self.render_mode as u32,
            b1: self.sellmeier_b[0],
            c1: self.sellmeier_c[0],
            b2: self.sellmeier_b[1],
            c2: self.sellmeier_c[1],
            b3: self.sellmeier_b[2],
            c3: self.sellmeier_c[2],
            _pad: [0.0; 2],
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

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Tonemapping Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.display_target.view,
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

            render_pass.set_pipeline(&self.tonemap_pipeline);
            render_pass.set_bind_group(0, &self.tonemap_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
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
            48,
            "RenderParams must be 48 bytes (3x 16-byte aligned blocks)"
        );
        Ok(())
    }

    #[test]
    fn test_headless_render_frame() -> TestResult {
        let fut = async {
            let ctx = match GpuContext::create_headless().await {
                Ok(c) => c,
                Err(RenderError::AdapterRequestFailed) => {
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

    #[test]
    fn test_headless_render_frame_bgra8unorm() -> TestResult {
        let fut = async {
            let ctx = match GpuContext::create_headless().await {
                Ok(c) => c,
                Err(RenderError::AdapterRequestFailed) => {
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

            // Metal macOS surface format: Bgra8Unorm
            let mut renderer = PathTracingRenderer::new(
                ctx,
                &scene,
                &camera,
                64,
                64,
                wgpu::TextureFormat::Bgra8Unorm,
            )?;

            assert_eq!(renderer.get_sample_count(), 0);
            renderer.render_frame();
            assert_eq!(renderer.get_sample_count(), 1);

            Ok(())
        };

        block_on(fut)
    }

    #[test]
    fn test_headless_render_tessellated_lens_multi_material() -> TestResult {
        let fut = async {
            let ctx = match GpuContext::create_headless().await {
                Ok(c) => c,
                Err(RenderError::AdapterRequestFailed) => {
                    return Ok(());
                }
                Err(e) => return Err(Box::new(e) as Box<dyn std::error::Error>),
            };

            let prescription = optics_core::OpticalPrescription {
                name: "Doublet System".to_string(),
                entrance_pupil_diameter_mm: 10.0,
                field_angles_deg: vec![0.0],
                wavelengths_nm: vec![587.56],
                primary_wavelength_index: 0,
                surfaces: vec![
                    optics_core::OpticalSurface {
                        surface_index: 1,
                        radius_mm: 45.0,
                        conic_constant: 0.0,
                        thickness_mm: 4.0,
                        semi_diameter_mm: 8.0,
                        material_name: "SK16".to_string(),
                        is_stop: false,
                        asphere_coefficients: Vec::new(),
                        coordinate_break: None,
                        grin_profile: None,
                        profile: None,
                        aperture: None,
                    },
                    optics_core::OpticalSurface {
                        surface_index: 2,
                        radius_mm: -30.0,
                        conic_constant: 0.0,
                        thickness_mm: 1.0,
                        semi_diameter_mm: 8.0,
                        material_name: "AIR".to_string(),
                        is_stop: false,
                        asphere_coefficients: Vec::new(),
                        coordinate_break: None,
                        grin_profile: None,
                        profile: None,
                        aperture: None,
                    },
                    optics_core::OpticalSurface {
                        surface_index: 3,
                        radius_mm: -30.0,
                        conic_constant: 0.0,
                        thickness_mm: 2.5,
                        semi_diameter_mm: 8.0,
                        material_name: "F2".to_string(),
                        is_stop: false,
                        asphere_coefficients: Vec::new(),
                        coordinate_break: None,
                        grin_profile: None,
                        profile: None,
                        aperture: None,
                    },
                    optics_core::OpticalSurface {
                        surface_index: 4,
                        radius_mm: -80.0,
                        conic_constant: 0.0,
                        thickness_mm: 40.0,
                        semi_diameter_mm: 8.0,
                        material_name: "AIR".to_string(),
                        is_stop: false,
                        asphere_coefficients: Vec::new(),
                        coordinate_break: None,
                        grin_profile: None,
                        profile: None,
                        aperture: None,
                    },
                ],
            };

            let (triangles, materials) =
                optics_bvh::prescription_to_trimesh_with_materials(&prescription, 16, 8);
            assert_eq!(materials.len(), 2);
            assert_eq!(materials[0], "SK16");
            assert_eq!(materials[1], "F2");

            let aabb = optics_bvh::Aabb::from_points(
                &triangles
                    .iter()
                    .flat_map(|t| [t.v0, t.v1, t.v2])
                    .collect::<Vec<_>>(),
            );
            let inst = optics_bvh::BlasInstance::new(0, &aabb, nalgebra::Matrix4::identity());
            let scene = BvhScene::build(vec![triangles], vec![inst]);

            let camera = ThinLensCamera::new(
                optics_core::Vec3::new(0.0, 0.0, -20.0),
                optics_core::Vec3::new(0.0, 0.0, 10.0),
                optics_core::Vec3::new(0.0, 1.0, 0.0),
                50.0,
                0.0,
                30.0,
            );

            let mut renderer = PathTracingRenderer::new(
                ctx,
                &scene,
                &camera,
                64,
                64,
                wgpu::TextureFormat::Rgba8Unorm,
            )?;

            // Assign per-material Sellmeier formulas: Material 0 = SK16, Material 1 = F2
            let gpu_mats = vec![GpuMaterial::sk16(), GpuMaterial::f2()];
            renderer.update_materials(&gpu_mats);

            assert_eq!(renderer.get_sample_count(), 0);
            for _ in 0..4 {
                renderer.render_frame();
            }
            assert_eq!(renderer.get_sample_count(), 4);

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
