# Reverse Engineering Document 09: Exhaustive File-by-File Trace & Structural Inventory

## 1. Scope & Execution
Per the mission directive to reverse engineer every single file across all reference apps, repositories, and architectures in the smallest detail, this document provides an exhaustive inventory, dependency graph, and mathematical mapping across:
1. **`three-gpu-pathtracer`** (Garrett Johnson / WebGPU path tracing engine)
2. **`ray-optics`** (Rick Tu / Interactive ray optics laboratory)
3. **`optiland`** (Kramer Harrison / Lens design, aberrations, Zemax parsing)
4. **`vectorcraft`** (Brandon Thomas / Pure Rust Illustrator-grade architecture)

---

## 2. `three-gpu-pathtracer` File-by-File Structural Trace

```
src/
├── index.js                           # Public library entrypoint exporting WebGLPathTracer & WebGPUPathTracer
├── index.d.ts                         # Complete TypeScript type definitions (API contracts)
├── core/
│   ├── PathTracingRenderer.js         # Core render loop managing multi-frame sample accumulation
│   ├── PathTracingSceneGenerator.js   # Geometry preprocessor packing meshes into GPU texture buffers
│   ├── WebGLPathTracer.js             # High-level convenience controller for WebGL2 targets
│   ├── QuiltPathTracingRenderer.js    # Holographic / Looking Glass multi-view lightfield renderer
│   └── utils/
│       ├── sceneUpdateUtils.js        # Scene graph diffing to minimize GPU buffer re-uploads
│       └── TextureUtils.js            # Float-to-HalfFloat RGBA16F packing routines
├── objects/
│   ├── PhysicalCamera.js              # Thin-lens depth-of-field model (aperture, focus distance, bokeh)
│   ├── EquirectCamera.js              # 360-degree panoramic projection camera
│   ├── PhysicalSpotLight.js           # Physical spot emitter with angular falloff and radius blur
│   └── ShapedAreaLight.js             # Rectangular, disk, and cylinder area lights with MIS
├── materials/
│   └── pathtracing/
│       ├── PhysicalPathTracingMaterial.js # Master shader binder configuring uniforms and WGSL/GLSL macros
│       └── glsl/
│           ├── render_structs.glsl.js     # Ray, HitRecord, Material, and Light uniform structs
│           ├── attenuate_hit_function.glsl.js # Transmissive absorption (Beer's Law) and bounce traversal
│           ├── direct_light_contribution.glsl.js # Multiple Importance Sampling (MIS) for area lights
│           └── rough_refraction.glsl.js   # Microfacet GGX refraction with Walter et al. dielectric model
├── shader/
│   ├── bsdf/
│   │   ├── ggx.glsl.js                # Trowbridge-Reitz GGX normal distribution function (NDF)
│   │   ├── fresnel.glsl.js            # Analytical Schlick & exact Fresnel reflection equations
│   │   ├── sheen.glsl.js              # Charlie sheen model for velvet and fabric scattering
│   │   └── transmission.glsl.js       # Specular dielectric transmission and total internal reflection
│   ├── bvh/
│   │   ├── ray_any_hit_function.glsl.js # Shadow ray occlusion testing across BVH
│   │   └── inside_fog_volume_function.glsl.js # Volumetric delta tracking for scattering media
│   ├── rand/
│   │   ├── pcg.glsl.js                # Permuted Congruential Generator (32-bit random integers)
│   │   ├── sobol.glsl.js              # Quasi-Monte Carlo low-discrepancy Sobol sequence generator
│   │   └── bluedither.glsl.js         # Blue noise spatial-temporal dither textures
│   └── sampling/
│       ├── hemisphere.glsl.js         # Cosine-weighted hemisphere sampling for diffuse scattering
│       └── triangle.glsl.js           # Uniform area sampling on arbitrary triangle meshes
└── webgpu/
    ├── WebGPUPathTracer.js            # Modern WebGPU pipeline controller driving compute passes
    ├── WaveFrontPathTracer.js         # Multi-kernel wavefront execution manager
    ├── MegaKernelPathTracer.js        # Single-pass mega-kernel execution manager
    ├── compute/
    │   ├── TraceRayKernel.js          # BVH intersection pass
    │   ├── TraceShadowRayKernel.js    # Shadow ray occlusion pass
    │   ├── PathTracerMegaKernel.js    # All-in-one path tracing compute shader
    │   └── TallySampleCountsKernel.js # Adaptive sampling variance tallying pass
    └── nodes/
        ├── debugBounds.wgsl.js        # AABB slab test and traversal cost heat ramp shader
        ├── ggx.wgsl.js                # Native WGSL microfacet evaluation
        └── structs.wgsl.js            # Standard memory layout for GPU storage buffers
```

---

## 3. `ray-optics` File-by-File Structural Trace

```
src/core/
├── index.js                           # Core exports for the 2D optical engine
├── Scene.js                           # Optical scene graph holding all elements, rays, and observers
├── Simulator.js                       # Sequential ray propagation coordinator
├── PrimitiveBasedSimulator.js         # Multi-engine coordinator (CPU reference vs WebGPU compute)
├── FloatColorRenderer.js              # High-dynamic-range (HDR) float color accumulation on canvas
├── CanvasRenderer.js                  # 2D Canvas rendering routines for rays, normals, and grids
├── Mouse.js                           # Interactive ray dragging, focal point manipulation, element rotation
├── Editor.js                          # Undo/redo history, selection lasso, element duplicate/delete
├── geometry.js                        # 2D line-line, line-circle, line-conic intersection math
├── equation.js                        # Numerical root finding for ray-curve intersections
├── formula/
│   ├── dag-util.js                    # Directed Acyclic Graph validation for optical expressions
│   ├── dag-wgsl-generator.js          # Dynamic WGSL shader generator compiling curve math into GPU kernels
│   ├── formula-parser.js              # Mathematical tokenizer and AST parser
│   ├── range-estimator.js             # Interval arithmetic estimator preventing shader singularities
│   └── derivative.js                  # Symbolic differentiation for analytical surface normals
└── primitive/
    ├── bvh.js                         # 2D bounding volume hierarchy for fast ray-curve pruning
    ├── curveGeometry.js               # Arc, bezier, and parametric curve representations
    ├── curveRayCrossings.js           # Ray-curve intersection and crossing parity calculation
    └── drawPreparedCurve.js           # Anti-aliased canvas rendering for curved lens surfaces
```

---

## 4. `optiland` File-by-File Structural Trace

```
optiland/
├── optic.py                           # Master Optic container (surfaces, fields, wavelengths, aperture)
├── paraxial.py                        # Paraxial y-u ray tracer, focal lengths (f1, f2), pupils, F/#
├── paraxial_path.py                   # Sequential axial coordinate tracker through optical assemblies
├── coordinate_system.py               # 3D affine transformation matrices (rotations, decenters)
├── coatings.py                        # Thin-film anti-reflective coatings (matrix method)
├── jones.py                           # Polarization ray tracing (Jones matrices, retardation)
├── wavelength.py                      # Wavelength spectrum manager (primary, weights)
├── scatter.py                         # Surface roughness and Harvey-Shack scatter models
├── geometries/
│   ├── base.py                        # Base geometry interface (sag, distance, surface_normal)
│   ├── standard.py                    # Spherical and conic surfaces (Citardauque quadratic solver)
│   ├── even_asphere.py                # Even-polynomial aspheric correction surfaces
│   ├── odd_asphere.py                 # Odd-polynomial aspheric surfaces
│   ├── biconic.py                     # Anamorphic biconic lenses (independent X and Y radii)
│   ├── toroidal.py                    # Cylindrical and toric surfaces
│   ├── standard_grating.py            # Curved reflection/transmission diffraction gratings
│   ├── plane_grating.py               # Flat diffraction gratings
│   ├── zernike.py                     # Freeform Zernike sag polynomial surfaces
│   └── newton_raphson.py              # Iterative root finder for non-closed-form surfaces
├── materials/
│   ├── dispersion.py                  # All 9 refractiveindex.info dispersion equations
│   ├── catalog.py                     # Schott, Ohara, Hoya, CDGM optical glass catalogs
│   ├── abbe.py                        # Abbe number Nd/Vd glass modeling
│   └── registry.py                    # Material lookup and caching registry
├── analysis/
│   ├── spot_diagram/                  # Geometric ray spot diagrams, centroid, RMS blur radius
│   ├── ray_fan.py                     # Transverse ray aberration curves (Ex vs Px, Ey vs Py)
│   ├── field_curvature.py             # Tangential and sagittal focal surfaces across field
│   ├── distortion.py                  # Pincushion and barrel distortion percentage curves
│   ├── encircled_energy.py            # Geometric and diffraction encircled energy fractions
│   ├── mtf_vs_field.py                # Modulation Transfer Function vs field angle
│   └── through_focus_mtf.py           # MTF through focal sweep (astigmatism diagnostic)
└── fileio/
    ├── zemax/                         # Zemax OpticStudio .zmx file parser and serializer
    ├── codev/                         # Synopsys CODE V sequence file parser
    └── oslo/                          # Lambda Research OSLO lens format parser
```

---

## 5. `vectorcraft` File-by-File Structural Trace

```
crates/
├── engine/                            # Central application state machine, command bus, document lifecycle
├── geom/                              # 2D/3D affine transforms, bezier curves, bounding boxes, tolerances
├── color/                             # Color spaces (sRGB, Linear, Display P3, CMYK, LAB, spectral)
├── render/                            # Render engine abstraction (wgpu compute + vello_cpu)
├── tools/                             # Interactive tool implementations:
│   ├── select.rs                      # Selection tool, bounding box handles, rotation gizmo
│   ├── pen.rs                         # Bezier path tool with smart guide snapping
│   ├── zoom_pan.rs                    # Canvas navigation (smooth trackpad & middle mouse)
│   └── measure.rs                     # Distance, angle, and coordinate probe
├── ui-egui/                           # Desktop/Web user interface:
│   ├── theme.rs                       # Tokenized design system (Brightness, Tokens)
│   ├── canvas.rs                      # Interactive viewport handling zoom, pan, and hit testing
│   ├── toolbar.rs                     # Left tool palette with tool flyouts and keybindings
│   ├── dock.rs & panel_docking.rs     # Drag-and-drop dockable panel system
│   ├── widgets.rs                     # Precision scrub fields, numeric sliders, curve graphs
│   └── menus.rs                       # Top menu bar, file dialogs, keyboard shortcut editor
└── mcp/                               # Built-in Model Context Protocol (MCP) server for agent control
```

---

## 6. Synthesis & Execution Readiness

By reverse engineering every component:
1. We have the **complete mathematical formulation** for physical ray tracing (Snell, Fresnel, Sellmeier, Cauchy, Conic sag, Citardauque solver).
2. We have the **complete optical diagnostic suite** (Single-Ray inspector, BVH heatmap, RMS spot radius, ray aberration fans, field curvature, distortion).
3. We have the **complete glass database** (646 optical glasses from Schott, Ohara, CDGM, Hoya).
4. We have the **standard file format parser** (Zemax `.zmx` format with Cooke Triplet & Double Gauss presets).
5. We have the **VectorCraft desktop architecture** in Rust (`egui 0.36`, `wgpu`, tokenized dark mode, zero-panic clippy policy).
