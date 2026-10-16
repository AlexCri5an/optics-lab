# Changelog

All notable changes to **OpticsLab** will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Changed

#### Desktop Native Consolidation & Reality Cleanup (`slice-g/native-cleanup`)
- **Single-Platform Desktop Native Focus:**
  - Removed `apps/optics-web` workspace crate, Trunk configuration, web assets, and all WASM dependencies (`wasm-bindgen`, `web-sys`, `js-sys`).
  - Pruned WASM build checks from GitHub Actions CI workflow to focus exclusively on native desktop workstation targets (Metal, Vulkan, DX12).
- **Physical Reality & Telemetry Corrections:**
  - Eliminated duplicate `r.render_frame()` invocation in 3D viewport paint callback and enforced strict `sample_count <= target_spp` cap.
  - Replaced CPU command encoding duration timer with wall-clock pacing, eliminating phantom 5000+ FPS reporting.
  - Removed synthetic $1/\sqrt{\text{SPP}}$ RMSE formula in Convergence dock, replacing it with transparent progress and honest "Complete (256/256 SPP)" status.
  - Disabled stubbed buttons (`Save Project`, `Export Image`, `Scale` tool) with informative reasons until native file dialogs and disk I/O are implemented.
  - Connected `Move` and `Rotate` tools directly to numeric translation/rotation controls in the Properties tab.
- **Dynamic WGSL Glass Dispersion:**
  - Replaced hardcoded N-BK7 Sellmeier coefficients in `crates/optics-render/src/shaders/path_tracer.wgsl` with dynamic uniform parameters passed via `RenderParams`.
  - Added `set_material_sellmeier` to `PathTracingRenderer` dynamically uploading active material coefficients.
  - Repaired 10× division typos for 4 Schott glasses in `optical_glasses.json` (`LITHOSIL-Q`, `N-LAF32`, `N-LASF31`, `N-LASF46`).
  - Unified optical glass naming across dock, inspector, and catalog to `"N-BK7 (Schott)"`.
- **Real 3D Viewport Scene Modes:**
  - Added `SceneModel::from_optical_prescription` to tessellate sequential lens systems into watertight 3D meshes with outward-facing normals.
  - Implemented 3D camera-facing ribbon propagation for sequential lens systems in the 3D viewport.
  - Bound lens simulation geometry and auto-framing ($Z \in [0, 50]$ mm) when switching to Lens System mode, loading presets, or dragging and dropping `.zmx` files.
- **Window Chrome & Typography:**
  - Overrode `clear_color` on `OpticsLabApp` (`eframe::App`) returning `self.tokens.panel_bg`, eliminating the solid black title bar gap on macOS.
  - Set minimum window inner size to 900x600 in `apps/optics-lab/src/main.rs`.
  - Enforced strict sentence case and removed trailing colons across all UI labels.

### Added

#### Realistic UI & Viewport Overhaul (`overhaul/realistic-ui`) — Phases P0–P11
- **Modern Minimal CAD Workstation Chrome (`optics-ui::theme`):**
  - Default CAD workstation theme (`ThemePreference::ClassicDark`) with flat 1 px borders, 0–2 px corner radii, zero decorative glow/shadows on chrome, and verified WCAG AA (≥4.5:1) contrast tokens.
- **Unified Command Architecture & State Synchronization (`optics-ui::commands`, `optics-ui::registry`):**
  - Exhaustive `enum Command` with 50+ strongly typed variants for all menus, toolbars, shortcuts, and panel actions.
  - Pure `dispatch` execution loop with deterministic `Effect` contracts and `Platform` abstraction.
  - Serializable `AppStateSnapshot` with stable 64-bit cryptographic hashing preventing dead/unwired UI controls.
- **Hardware-Accelerated WebGPU 3D Viewport (`optics-ui::viewport_3d`, `optics-render`):**
  - Switched `eframe` to `"wgpu"` with native progressive path tracing callbacks via `egui_wgpu`.
  - Orbit camera with pan, tilt, zoom, and distance controls.
  - Adaptive status bar reporting real GPU adapter info, graphics backend, accumulated SPP, and render frame latency.
- **Parametric 3D Scene Model & Geometry Tessellation (`optics-ui::scene_model`):**
  - Pure procedural watertight meshes for Prisms, UV Spheres, Optical Slabs, Singlet Lenses, and Floor Planes with calculated outward normals.
  - Interactive scene hierarchy with Add, Duplicate, Delete, Visibility, and Selection.
  - Live 2-level BVH (TLAS/BLAS) rebuilds with sub-millisecond build-time telemetry.
- **Physical Beam Propagation & Caustics (`optics-ui::beam_ribbon`):**
  - 3D camera-facing billboard ribbons for incoming collimated beams and dispersed spectral rays.
  - Full CIE 1931 XYZ color matching and IEC 61966-2-1 piecewise sRGB transfer.
  - Floor spectral caustic footprints with nanometer wavelength ruler.
  - Dielectric group velocity time-of-flight pulses demonstrating dispersion in the time domain ($v_g = c / n(\lambda)$).
- **Live Scientific Telemetry Dock (`optics-ui::analytics`):**
  - Replaced static constants and dummy loops with 4 live cards:
    - Convergence card: Live Welford variance, RMSE, and 95% confidence intervals.
    - Spectral Power Distribution card: Real hero-wavelength spectrum preview.
    - BVH Diagnostics card: Real ray traversal cost readback and benchmark against active scene.
    - Material / Glass card: Live Sellmeier dispersion curve and interactive material editor.
- **Screen Boundary Enforcement & Geometric Layout Assertions (`optics-ui::layout`, `tests/geometric_layout_tests.rs`):**
  - Bounded panel dimensions with scroll areas (`vscroll(true)`) preventing clipping across resolutions from 900x600 to 4K.
  - Replaced churn-prone pixel snapshots with mathematical non-overlap assertions (`!r1.intersect(r2).is_positive()`).
  - Guaranteed touch targets $\ge 40$ px in touch mode.
- **Comprehensive Physics & Rendering Validation Suite (`tests/physics_validation_tests.rs`):**
  - Verified GPU single-precision Sellmeier vs CPU double-precision prism angle deviation ($\Delta \theta \le 0.05^\circ$).
  - Verified glass-slab multi-reflection energy conservation ($R + T = 1.0 \pm 10^{-10}$).
  - Verified White Furnace equilibrium radiance conservation ($L_o = 1.0 \pm 10^{-10}$).
  - Verified sub-frame recalculation performance ($\ll 16$ ms).

#### Phase 12.3: WASM Drag-and-Drop & Default Scene Loading (`optics-ui`, `optics-web`)
- **Interactive Drag-and-Drop .zmx Prescription Loading (`optics-ui::app`):**
  - Added `handle_file_drag_and_drop` supporting direct file drops onto the application window and WASM canvas.
  - Automatically parses `.zmx` bytes via `optics_zemax::parse_zmx_bytes`, extracts the `OpticalPrescription`, and dynamically updates `LensSimulation`.
  - Added drag-and-drop hover feedback banner ("📂 Drop .zmx file to load optical prescription") in the central viewport toolbar.
  - Integrated preset loading and drag-and-drop user guidance into the top `File` menu.
- **Embedded Cooke Triplet Default WASM Scene (`optics-ui::app`, `apps/optics-web`):**
  - Configured target-specific default state for `wasm32-unknown-unknown` opening directly in `AppMode::LensSystem` with canonical Cooke Triplet F/4.5 preset, 2D cross-section layout, and prescription editor.
- **Dynamic Prescription Loading (`optics-ui::lens_sim`):**
  - Added `load_prescription_custom` and `from_prescription` to `LensSimulation` for loading arbitrary user prescriptions with full recalculation.

#### Phase 11 & Phase 14.1: CLI Diagnostic Tools & End-to-End Integration Suite (`optics-cli`, `optics-zemax`)
- **Optical Glass Material Diagnostics (`optics-cli glass`):**
  - Added `glass --name <name> [--json]` subcommand to inspect dispersion equations, Sellmeier resonance coefficients, and Fraunhofer line refractive indices ($F, d, C, e, g$).
  - Outputs Abbe number $V_d$, principal/partial dispersion ratios, optical glass region classification (Crown vs Flint), visible spectrum table ($380\dots780\,\text{nm}$), and ASCII Abbe coordinate location.
  - Supports full JSON structured output for automated pipelines.
- **BVH Ray Traversal Benchmark (`optics-cli benchmark`):**
  - Added `benchmark [--scene <scene>] [--rays <N>] [--json]` subcommand running multi-level BVH slab intersections against 3D optical meshes.
  - Measures BVH build latency, total ray traversal duration, intersection hit rates, throughput in MRAYS/s, and average traversal latency in ns/ray.
- **Spot Diagram CSV Export (`optics-cli analyze --csv`):**
  - Added `--csv <path>` flag to `optics-cli analyze` exporting per-ray pupil and image coordinate data ($P_x, P_y, x, y, \text{intensity}, \lambda$) for external analysis.
- **End-to-End Integration Test Suite (`crates/optics-zemax/tests/e2e_integration_tests.rs`):**
  - `test_e2e_cooke_triplet_efl_within_0_1_percent`: Verifies that parsing canonical Cooke Triplet `.zmx` and calculating paraxial properties produces EFL within 0.1% of nominal 50.0 mm (verified 50.021 mm, 0.042% error).
  - `test_e2e_cooke_triplet_spot_rms_under_50um`: Traces hexapolar spot diagram on Cooke Triplet at 0° field and asserts RMS blur radius $< 50\,\mu\text{m}$ (verified 4.70 µm).
  - `test_e2e_round_trip_zmx_all_presets`: Verifies round-trip parse $\to$ write $\to$ parse across Cooke Triplet, Double Gauss, and Plano-Convex presets with identical optical surface geometries and materials.

#### Phase 9.5 & 9.8: Glass Catalog Browser, Abbe Diagram & Interactive Widgets (`optics-ui`, `optics-glass`)
- **Technical UI Widgets (`optics-ui::widgets`):**
  - `scrub_field`: Drag-to-edit numerical parameter field with fine/coarse drag sensitivity, hover state feedback, and boundary clamping.
  - `dispersion_curve_plot`: Continuous refractive index $n(\lambda)$ curve from $380\dots780\,\text{nm}$ marked with Fraunhofer $F$ (486.1 nm), $d$ (587.6 nm), and $C$ (656.3 nm) spectral lines.
  - `abbe_diagram`: Interactive Abbe $V_d$ vs $n_d$ scatter plot with standard optical glass coordinate convention (inverted $V_d \in [20, 85]$, $n_d \in [1.45, 2.05]$), crown/flint boundary line, and active selection ring.
- **Glass Catalog Browser Panel (`optics-ui::glass_browser`):**
  - Searchable catalog inspector filtering across all 646 bundled glasses from Schott, Ohara, CDGM, and Hoya.
  - Summary card displaying refractive index $n_d$, Abbe number $V_d$, and Sellmeier oscillator resonance coefficients.
  - Live dispersion curve and Abbe scatter plot integrated directly into the browser tab.
  - Added `all_glasses()` iterator on `GlassCatalogDb` and `name()` method on `GlassCatalog`.
- **Keyboard Shortcuts & Help Modal (`optics-ui::app`):**
  - Keyboard shortcuts cheat sheet dialog (`F1` or Help menu) documenting navigation, tool keys, parameter scrubbing, and viewport shortcuts.
  - Right inspector tab switcher integrating Ray Inspector, Lens Prescription Table Editor, and Glass Catalog Browser.
- **GPU Hardware Context & Target Allocations (`optics-render::context`, `optics-render::target`):**
  - `GpuContext` with headless adapter/device initialization and surface integration.
  - `RenderTarget` (Rgba16Float HDR accumulation buffer) and `ToneMapTarget` with `resize_render_targets`.
- **Physical Camera Models (`optics-render::camera`):**
  - `ThinLensCamera` with depth-of-field, aperture diameter, focal plane distance, concentric disk lens sampling, and 16-byte aligned `CameraUniform`.
  - `OrthographicCamera` for technical paraxial cross-sections and layout views.
- **BVH Buffer Upload Pipeline (`optics-render::bvh_upload`):**
  - `BvhBuffer` uploading flat 16-byte aligned BVH node arrays and triangle primitive pools directly into GPU storage buffers.
- **WGSL Compute & Render Shaders (`optics-render::shaders`):**
  - `path_tracer.wgsl`: PCG32 random number generator, thin-lens camera ray generation, iterative BVH traversal (32-deep stack), Fresnel dielectric reflection/refraction, Total Internal Reflection (TIR), and Russian Roulette path termination.
  - `tonemapping.wgsl`: Fullscreen vertex generator, ACES filmic tone mapping operator, and IEC 61966-2-1 piecewise sRGB gamma transfer curve.
  - `bvh_heatmap.wgsl`: Traversal complexity visualizer with smooth temperature ramp ($Blue \to Cyan \to Green \to Yellow \to Red$).
- **Statistical Convergence Analytics (`optics-render::convergence`):**
  - `ConvergenceTracker` implementing Welford's online one-pass mean and variance algorithm ($\sigma^2, \text{SEM}$).
  - `compute_rmse` for root-mean-square error tracking against reference solutions.
- **Renderer Orchestration (`optics-render::pipeline`):**
  - `PathTracingRenderer` dispatching compute passes, managing uniform bindings, and incrementing sample accumulation.

#### Phase 3.3: Hero-Wavelength Spectral Sampling (`optics-spectral`)
- **Stratified Hero-Wavelength Engine (`optics-spectral::hero`):**
  - `HeroSpectrum<const N: usize>` and `DynamicHeroSpectrum` for stratified Monte Carlo spectral ray sampling in $380\dots780\,\text{nm}$.
  - Primary hero wavelength selection with uniform stratification, wrap-around, and relative sample weights.
  - Direct spectral integration estimators: `to_xyz`, `to_linear_srgb`, `to_srgb`, and `d65_weights`.
  - Constant spectrum test confirming flat white reconstruction across stratified hero bundles.

#### Phase 6: 2-Level BVH Acceleration Structure & GPU Buffer Packing (`optics-bvh`)
- **Bounding Volume Hierarchy (`optics-bvh::aabb`, `optics-bvh::builder`):**
  - Axis-Aligned Bounding Box (`Aabb`) with Kay-Kajiya / Smits ray-slab intersection test handling zero directions, NaNs, and grazing incidence safely.
  - 16-bin Surface Area Heuristic (SAH) builder (`build_bvh`, `find_best_split`) with spatial median partitioning fallback.
- **2-Level TLAS / BLAS Architecture (`optics-bvh::scene`, `optics-bvh::triangle`):**
  - Bottom-Level Acceleration Structure (BLAS) accelerating triangular meshes via Möller–Trumbore intersection.
  - Top-Level Acceleration Structure (TLAS) over `BlasInstance` elements with affine local-to-world transformation matrices.
  - Hierarchical ray traversal (`BvhScene::intersect`) with world-to-local ray transformation and outward normal preservation.
- **GPU Buffer Layout & Memory Alignment (`optics-bvh::gpu`):**
  - `GpuBvhNode`: 32-byte struct with 16-byte alignment (`#[repr(C, align(16))]`) implementing `bytemuck::Pod` and `Zeroable`.
  - `GpuTriangle`: 64-byte struct with 16-byte alignment (`v0, v1, v2, normal_mat`) implementing `bytemuck::Pod` and `Zeroable`.
  - Zero-copy buffer serialization routines: `pack_bvh_to_gpu_buffer` and `pack_triangles_to_gpu_buffer`.
- **Traversal Diagnostics & Cost Heatmaps (`optics-bvh::cost`):**
  - `trace_bvh_cost` measuring intersection evaluation count for performance diagnostics and heatmaps.

#### Scientific Desktop Workstation UI (1:1 Reference Layout & Visual Language)
- **Design Tokens & Theme System (`optics-ui::theme`):**
  - Canonical dark scientific palette matching reference: `#080D13` (canvas), `#0D141C` (panels), `#101820` (secondary), `#26313B` (borders), `#1B252E` (subtle borders), `#D8DEE6` (primary text), `#8995A3` (secondary text), `#5E6A76` (muted), `#202B36` (selected item), `#A9B8C8` (accent).
  - WCAG AA contrast ratio compliance verified by automated unit tests ($11.8:1$ primary text, $6.1:1$ secondary text).
- **Top Application Bar (`optics-ui::app`):**
  - Native desktop title & menu bar with OpticsLab prism logo, application title, dot separator, menus (`File`, `Edit`, `View`, `Render`, `Diagnostics`, `Help`), `128 spp` frame indicator, and window controls (minimize, maximize, close).
- **Left Sidebar Tools & Scene Hierarchy (`optics-ui::app`, `optics-ui::icons`):**
  - Tool list with 10 tools (`Select`, `Move`, `Rotate`, `Scale`, `Ray Inspector`, `Spectrum`, `BVH`, `Analysis`, `Camera`, `Settings`) drawn with custom outline vector icons and 2px rectangular selection highlight.
  - Collapsible Scene hierarchy tree: `Scene` root with disclosure arrows, `Camera`, `Prism` (expandable to `Glass (BK7)` and `Material`), `Light`, and `Ground`.
- **Center 3D Perspective Viewport (`optics-ui::viewport_3d`):**
  - Realistic 3D perspective projection of the upright glass prism on dark floor grid.
  - White collimated incident ray, physical internal refraction, and emerging luminous spectral fan (violet, blue, cyan, green, yellow, orange, red) with glow halo and floor projection.
  - Lower-left 3-axis coordinate gizmo with labeled X (red), Y (green, up), and Z (blue, depth) arrows.
  - Viewport mode switcher: `[ 3D View ]` (active), `[ 2D View ]`, `[ Spectral ]`, `[ BVH ]`, `[ Convergence ]` plus Screenshot, Frame, and Fullscreen icon buttons.
  - Click-to-inspect interactive ray picking directly from the 3D viewport canvas.
- **Embedded Scientific Analytics Dock (`optics-ui::analytics`):**
  - **Convergence:** Semi-log RMSE (cyan) and Variance (amber) decay curves with sample axis ($0 \dots 256$) and error axis ($0.01 \dots 1.0$).
  - **Spectral Power:** Physical wavelength histogram ($380 \dots 700\,\text{nm}$) colored per-bin with continuous rainbow spectral gradient strip.
  - **BVH Diagnostics:** Wireframe bounding box hierarchy with traversal cost heatmap, vertical colorbar (High to Low), TLAS/BLAS indicators, and traversal statistics (Median 43, P95 91, Max 184).
  - **Material / Glass:** BK7 (Schott) dropdown, physical parameters ($n$, Abbe $V_d$, Cauchy dispersion, Sellmeier), miniature dispersion curve, and Edit Material button.
- **Right Ray Inspector Panel (`optics-ui::inspector`):**
  - Detailed physical diagnostics: Pixel coordinates `Pixel (842, 412) -> Ray #1842`, wavelength readout $\lambda = 587.56\,\text{nm}$ (green) with color swatch dot.
  - Expandable per-bounce physical parameter tables: Surface, $n_1, n_2, \theta_i, \theta_t$, Fresnel $R_s, R_p, R_{\text{total}}, T_{\text{total}}$, and Event type.
  - Vertical ray path node graph from Camera through refractive/reflective surfaces to Light emitter.
- **Bottom Status Bar (`optics-ui::app`):**
  - System status readout (`Ready`) and real-time telemetry (`128 spp | 1920x1080 | GPU: Vulkan | 60 FPS`).

#### Slice B, Step B4: Prescription Editor UI, Aberration & Spot Plots, and CLI Diagnostics (`optics-ui`, `optics-cli`)
- **Lens System & Aberration Analysis State Model (`optics-ui::lens_sim`):**
  - `LensSimulation` state engine supporting canonical presets (`Cooke Triplet F/4.5`, `Double Gauss F/2.0`, `Plano-Convex Singlet`) and dynamic custom prescriptions.
  - Automatic calculation of paraxial properties (EFL, BFL, working F/#, entrance/exit pupil positions), 2D cross-section ray fan paths, geometric spot diagrams, and transverse aberration fans.
  - Fast recalculation performance ($\approx 1.2\,\text{ms}$ on Cooke Triplet), well within the $16.6\,\text{ms}$ 60 FPS frame budget.
- **Academic Minimalist UI Workbench (`optics-ui::lens_view`):**
  - Classic academic toolbar inspired by Microsoft Word and classic optical engineering tools (Zemax, AutoCAD).
  - Four dedicated subviews:
    - **Layout & Rays:** 2D cross-section rendering with glass elements, optical axis, aperture stop marker, image sensor, and traced meridional ray paths.
    - **Spot Diagram:** 2D geometric spot diagram scatter plot with Airy disk circle, RMS blur circle, centroid crosshair marker, and concentric micrometer grid.
    - **Aberration Fans:** Tangential $E_y(P_y)$ and sagittal $E_x(P_x)$ transverse ray aberration curves on high-contrast academic grid.
    - **Prescription Table:** Editable tabular lens editor with live drag adjustments for radius, thickness, glass material, semi-diameter, and conic constant.
  - Left diagnostic card displaying paraxial first-order properties, spot diagram metrics, and transmission/vignetting percentage.
  - Quick field angle selection ($0^\circ, 10^\circ, 14^\circ$) and Fraunhofer wavelength switching (d, F, C).
  - Classic 2px corner radius styling adhering strictly to WCAG AA contrast standards.
- **Headless Lens System Diagnostics CLI (`apps/optics-cli analyze`):**
  - CLI subcommand `optics-cli analyze` evaluating bundled presets or arbitrary external `.zmx` files.
  - Comprehensive report output including paraxial properties, spot metrics (Airy disk, RMS blur, diffraction-limited verdict), and tangential fan samples.
  - Machine-readable `--json` output format for automated testing and CI pipelines.
- **Snapshot & Harness Test Suite (`crates/optics-ui/tests/snapshot_tests.rs`):**
  - Headless egui harness snapshot tests validating all four subviews at $900 \times 600$ minimum resolution at $1\times$ and $2\times$ DPI.

#### Slice B, Step B3: Geometric Spot Diagrams & Transverse Ray Aberration Fans (`optics-core`, `optics-zemax`)
- **Pupil Distribution Sampling (`optics-core::analysis`):**
  - `PupilPattern` supporting `Hexapolar`, `Rectangular`, and `Cross` sampling grids in normalized pupil coordinates $(P_x, P_y) \in [-1, 1]$ with $P_x^2 + P_y^2 \le 1$.
  - Exact hexapolar ring count formulation ($N = 1 + 3 R (R + 1)$) generating uniformly distributed hexagonal rings with 6-fold radial symmetry.
  - `launch_pupil_ray` launching real 3D rays from field angles $(H_x, H_y)$ through normalized pupil coordinates $(P_x, P_y)$ targeted at the paraxial entrance pupil.
- **Geometric Spot Diagram Analyzer (`optics-core::analysis`):**
  - `trace_spot_diagram` tracing multi-ring pupil grids through sequential systems to the image sensor plane.
  - Statistical spot metrics via `spot_diagram_metrics`: centroid coordinates $(\bar{x}, \bar{y})$, RMS blur radius $\sigma_r = \sqrt{\frac{1}{N} \sum (x_i - \bar{x})^2 + (y_i - \bar{y})^2}$, geometric 100% enclosure radius, and Airy disk diffraction limit ($r_{\text{Airy}} = 1.22 \lambda F/\#$).
  - Diffraction-limited criterion check ($\text{RMS} \le r_{\text{Airy}}$).
- **Transverse Ray Aberration Fans (`optics-core::analysis`):**
  - `trace_ray_fan` evaluating transverse ray aberration errors $\epsilon_y(P_y)$ (tangential fan, $P_x = 0$) and $\epsilon_x(P_x)$ (sagittal fan, $P_y = 0$) relative to the chief ray in micrometers ($\mu\text{m}$).
  - Chief ray intersection determination and per-ray aperture vignetting detection.
- **Analytical & System Invariant Validations:**
  - Plano-hyperbolic Cartesian aplanat ($k = -n^2$): verified exact geometric stigmatism with spot RMS radius $< 10^{-6}\,\mu\text{m}$ and zero transverse ray aberration.
  - Spherical singlet cubic aberration scaling: verified odd tangential symmetry ($\epsilon_y(-P_y) = -\epsilon_y(P_y)$) and cubic $P_y^3$ scaling ($\approx 8\times$ ratio between $P_y = 1.0$ and $P_y = 0.5$).
  - Chromatic ray fan slope difference: verified blue Fraunhofer F ($486.13\,\text{nm}$) ray fan lands below red Fraunhofer C ($656.27\,\text{nm}$) on paraxial d-focus, confirming positive longitudinal chromatic aberration.
  - Cooke Triplet F/4.5 ($f = 50\,\text{mm}$): verified on-axis RMS blur ($\approx 19.5\,\mu\text{m}$) and off-axis ($14^\circ$) spot diagram with physical vignetting ($83 / 127$ hexapolar rays passing without barrel clipping, $\approx 65.4\%$ transmission).

#### Slice B, Step B2: Zemax `.zmx` Parser & Canonical Presets (`optics-zemax`, `optics-glass`)
- **Zemax .zmx Format Parser (`optics-zemax::parser`):**
  - Robust parser `parse_zmx` supporting standard Zemax operands: `NAME`, `ENPD`, `FNUM`, `WAVL`, `WAVM`, `PWAV`, `FTYP`, `YFLD`, `YFLN`, `SURF`, `TYPE`, `CURV`, `DISZ`, `GLAS`, `DIAM`, `CONI`, `STOP`.
  - Byte-slice parser `parse_zmx_bytes` with automatic detection and decoding of UTF-16 LE BOM (`0xFF 0xFE`) or UTF-8.
  - Curvature to radius conversion handling flat surfaces ($c \to 0 \implies R = \infty$).
  - Extensible `SurfaceType` enum (`Standard`, `EvenAsphere`, `Toroidal`, `ZernikeSag`).
  - Structured, line-numbered `ZemaxParseError` with contextual operand details.
- **Zemax .zmx Serializer (`optics-zemax::serializer`):**
  - `write_zmx` and `write_prescription_zmx` providing complete round-trip text serialization.
  - Round-trip fidelity verified by test: parse $\to$ write $\to$ re-parse yields numerically equivalent systems.
- **Canonical Lens Presets (`optics-zemax::presets`):**
  - **Cooke Triplet F/4.5 ($f = 50\,\text{mm}$):** Classical 3-element, 6-surface design eliminating 5 primary Seidel aberrations. Verified paraxial $\text{EFL} = 50.0 \pm 0.5\,\text{mm}$ ($50.00\,\text{mm}$), $\text{BFL} = 42.21 \pm 0.5\,\text{mm}$, and real sequential ray trace focusing on sensor.
  - **Double Gauss F/2.0 ($f = 50\,\text{mm}$):** 6-element, 4-group configuration forming the basis of standard 35 mm SLR camera lenses. Verified paraxial $\text{EFL} = 50.0 \pm 1.0\,\text{mm}$ and F-number $2.0 \pm 0.1$.
  - **Plano-Convex N-BK7 ($f = 100\,\text{mm}$):** Singlet with verified paraxial $\text{EFL} = 100.0\,\text{mm}$ and thick-lens $\text{BFL} = 96.7\,\text{mm}$.
- **Glass Catalog Material Resolver Bridge (`optics-glass::catalog`):**
  - Implemented `optics_core::MaterialResolver` for `GlassCatalogDb`.
  - Added case-insensitive alias and `N-` prefix fallback (e.g. `SK16` resolves to `N-SK16`, `N-F2` resolves to `F2`).
  - Allows `prescription.resolve(&catalog_db)` to resolve all 646 catalog glasses into thread-safe `Arc<dyn RefractiveIndex>`.

#### Slice B, Step B1: Sequential Optics Core (`optics-core`)
- **Optical Conventions Documentation (`docs/conventions.md`):**
  - Documented spatial coordinates in mm, $+z$ propagation direction, curvature $c = 1/R$, conic sag equation, normal convention pointing toward incident medium, and Citardauque rearranged root solver.
- **Conic Surface Geometry (`optics-core::conic`):**
  - `ConicSurface` struct parameterized by curvature $c = 1/R$, conic constant $k$, semi-diameter, and axial vertex position $z_v$.
  - Numerically stable `conic_sag(r, c, k)` with finite boundary checking for $(1+k) c^2 r^2 > 1$, returning typed [`OpticsError::GeometryOutsideAperture`].
  - Analytic unit normal `conic_normal` oriented toward the incident medium ($\mathbf{n} \cdot \hat{z} \le 0$).
  - Citardauque quadratic root solver `intersect_conic` preventing catastrophic cancellation when $c \to 0$ or ray is near-axial, with vertex-sheet validation and semi-diameter aperture clipping.
- **Sequential Prescriptions & Materials (`optics-core::prescription`):**
  - `OpticalSurface` and `OpticalPrescription` representing sequential surface definitions (radius, conic, thickness, semi-diameter, material name, stop flag).
  - Decoupled `ResolvedPrescription` and `ResolvedSurface` using `trait MaterialResolver` returning `Arc<dyn RefractiveIndex + Send + Sync>`, ensuring zero dependency on `optics-glass`.
- **Paraxial Ray Tracer (`optics-core::prescription`):**
  - $y$-$u$ matrix method: `trace_paraxial_marginal` and `trace_paraxial_chief` tracking refraction ($n' u' = n u - y \phi$) and axial transfer ($y' = y + t u'$).
  - `calculate_paraxial_properties` computing Effective Focal Length (EFL), Back Focal Length (BFL), F-number, and pupil locations.
- **Sequential Real-Ray Tracer (`optics-core::prescription`):**
  - `trace_sequential` tracing rays surface-by-surface with conic intersection, Snell refraction, TIR detection, vignetting enforcement, and per-surface Fresnel efficiency recording in `RayBounce`.
- **Analytical Unit Tests (Derived values, zero invented numbers):**
  - Conic surface satisfaction to $< 10^{-12}$, circle intersection equivalence, parabola sag known value $r^2/(2R)$, and normal orthogonality to finite-difference tangents.
  - Paraxial thin-lens and thick-lens equivalence to lensmaker equations, plano-convex EFL/BFL relations, two-surface combination, and Helmholtz-Lagrange invariant conservation across all surfaces.
  - Real-ray $h \to 0$ convergence to paraxial BFL, longitudinal spherical aberration (LSA) negative sign and $\sim h^2$ scaling, Cartesian plano-hyperbolic aplanat exact focus ($k = -n^2$, focal spread $< 10^{-9}$ mm), chromatic focal order, Fresnel throughput conservation, and ray reversibility.

#### Slice A: Interactive Prism Dispersion UI (`optics-ui`, `optics-lab`, `optics-web`)
- **Theme & Design Tokens (`optics-ui::theme`):**
  - `OpticsTokens` with MediumDark palette (canvas `#11141a`, panels `#1a1f2e`, accent `#00d4f5`, amber `#f5a623`, dim text `#8b93a7`, text `#e6e9f0`).
  - `apply_tokens(ctx, tokens)` configuring typography, spacing, widget interactions (hover/active/selection), and separators.
  - Unit tests verifying WCAG AA contrast ratio ($\ge 4.5:1$) for both primary and dim text against panels and canvas.
- **Pure View Transform (`optics-ui::view_transform`):**
  - `ViewTransform` supporting world-to-screen and screen-to-world conversions, panning, cursor-anchored zoom, and bounds fitting.
  - Comprehensive unit tests covering round-trip conversions, cursor-invariance during zoom, and margin-padded bounds fitting.
- **Hit Testing & Ray Selection (`optics-ui::hit_test`):**
  - Pure screen-space `point_to_segment_distance` and `find_hovered_ray_index` with configurable pixel threshold.
  - Unit tests covering segment endpoints, interior projections, degenerate segments, and ties.
- **Formatting Helpers (`optics-ui::formatting`):**
  - Pure string formatters for angles (2 decimals), percentages (1 decimal), refractive indices (6 decimals), and wavelengths (1 decimal).
- **Prism Simulation & Validation (`optics-ui::sim`):**
  - `PrismSimulation` calculating 2D prism geometry, apex and incidence angle modifications, and tracing spectral fans across glass presets (`N-BK7`, `SF11`, `F2`, `Fused Silica`).
  - Unit test verifying the UI's deviation helper for 587.56 nm equals the two-surface analytic Snell deviation within $10^{-9}$ and matches the CLI reference output ($38.9567^\circ$).
- **OpticsLabApp (`optics-ui::app`):**
  - Top menu bar: `File > Quit`, `View > Reset view`, `Help > About`.
  - Left tool strip: Select (S) and Ray probe (I, active by default); planned tools disabled with "Coming later" tooltips.
  - 2D Canvas: rendered exclusively via `egui::Painter` with subtle prism fill, outline, incoming D65 ray with label, and spectral fan colored by wavelength.
  - Right Inspector Panel: Glass dropdown, apex angle slider ($20^\circ$–$90^\circ$), incidence angle slider ($0^\circ$–$80^\circ$), sample count slider ($8$–$64$), $n_d$/$V_d$ readouts, angular spread readout, and per-bounce `RayBounce` inspection table.
  - Hover highlights nearest ray; left-click selects with interaction dots; Esc deselects; double-click resets view.
  - Empty state handling: Displays guidance message when TIR prevents rays from exiting prism; TIR events marked in inspector.
- **Native Desktop Application (`apps/optics-lab`):**
  - Configured `eframe::run_native` runner with window title "OpticsLab", minimum size 900x600, default size 1200x800.
- **WebAssembly & Trunk Setup (`apps/optics-web`):**
  - Configured `Trunk.toml` and `index.html` with full-window canvas, animated loading spinner, and plain-text WebGL/WebAssembly availability checks.
  - Built and verified release WASM bundle (`3.20 MB` raw, `1.44 MB` gzip).

#### Slice A, Step A2: UI Visual & Interaction Polish
- **Minimalist CAD Vector Icons (`optics-ui::icons`):**
  - Custom vector icons for Select, Ray Probe, Lens Surface, Light Source, and Measure drawn directly with `egui::Painter`.
  - Contrasts verified $\ge 3:1$ across normal, hover, active, and disabled states (`test_tool_icon_contrast_ratios`).
  - Replaced non-ASCII symbols with clean ASCII characters verified by character set restriction test (`test_ui_allowed_character_set`).
- **Scene Auto-Fit & Finite Rays (`optics-ui::sim`):**
  - Pure `compute_scene_bounds` enclosing prism vertices, incoming ray source origin, and all ray/branch endpoints.
  - Unit tests verifying fit containment across extreme apex ($20^\circ$ and $90^\circ$) and incidence ($0^\circ$ and $80^\circ$) angles.
- **Translucent Optical Prism Styling (`optics-ui::app`):**
  - Low-alpha glass tint mapped from $n_d$ (`Color32::from_rgba_unmultiplied(32, 44, 64, tint_alpha)`), thin lighter outline, inner facet highlight, and muted base facet.
- **Polished Rays & Fresnel Reflection Branches (`optics-ui::app`, `optics-ui::sim`):**
  - Incoming ray with source emitter dot and "D65 white light" label.
  - Layered spectral fan rays with soft glow underlay + crisp core; hover wavelength tooltip near cursor.
  - Selected ray styling: retains spectral color, wide glow underlay, white-ringed bounce dots, and terminal wavelength badge.
  - Dashed Fresnel reflection branches with alpha mapped from $R_\text{total}$ via pure `map_reflectance_to_alpha`.
- **Wavelength Legend Bar (`optics-ui::legend`):**
  - Pure bidirectional wavelength-to-pixel coordinate mapping with tick marks every 100 nm, sample dots, selected ray marker, and interactive click selection.
- **Inspector Polish, Copy Table & Controls (`optics-ui::app`):**
  - Sentence case headings, no trailing colons, monospace right-aligned numbers.
  - Two-row wrapped bounce layout fitting panel width without horizontal scrolling.
  - "Copy table" TSV clipboard export with user feedback.
  - Sliders with double-click to reset, scene "Reset scene" button, and glass selector showing $n_d$ readout.
  - Keyboard shortcuts: `S`, `I`, `Esc`, `R` / `Cmd+0`.
- **Edge State Detection (`optics-ui::sim`):**
  - Pure `EdgeState` detection (`AllTotalInternalReflection`, `NoRaysTransmitted`, `RayMissedPrism`) with visible canvas banners.
- **Automated UI Harness Tests (`optics-ui/tests/snapshot_tests.rs`):**
  - `egui_kittest` execution tests at 1.0 and 2.0 DPI for default state, selected ray state, and TIR state.

---

## [0.1.0-research] — 2026-10-15

### 🔬 Reverse Engineering & Research Phase (Complete)

This milestone represents the completion of the full REA (Reverse Engineering & Analysis) phase
before any implementation begins. All mathematical foundations, architectural blueprints, glass
catalogs, and file format specifications have been extracted and documented.

#### Added — Reference Repositories Cloned

- **`ray-optics`** (Rick Tu) — interactive 2D optical simulator; extracted scene graph, Snell's Law
  vector forms, 2D BVH, and symbolic derivative surface normals.
- **`three-gpu-pathtracer`** (Garrett Johnson) — WebGPU path tracer; extracted GGX microfacet BSDF,
  Schlick Fresnel, MIS area lights, 2-level BVH slab intersection, Sobol quasi-random sampler,
  wavefront kernel architecture, and WGSL traversal cost heatmap shader.
- **`optiland`** (Kramer Harrison) — Python optical design library; extracted full paraxial y-u ray
  transfer matrix method, sequential surface ray tracer, 9 refractiveindex.info dispersion formulas,
  conic/aspheric sag equations (Citardauque solver), Zemax `.zmx` parser, CODE V parser, and the
  complete analysis suite (spot diagram, ray fan, MTF, encircled energy, distortion).
- **`vectorcraft`** (Brandon Thomas) — pure-Rust creative application; extracted workspace
  architecture (`crates/` + `apps/`), tokenized egui design system, dockable panel pattern,
  zero-panic clippy policy, and wgpu render abstraction.
- **`rea`** (morluto) — REA methodology guide; used for structured evidence extraction across all
  JavaScript bundles, WebGPU shaders, and Python bytecode.

#### Added — Reverse Engineering Documents

| # | Document | Contents |
|---|----------|----------|
| 01 | `01_single_ray_inspector.md` | Snell's Law 3D vector form, Fresnel Rs/Rp, TIR, per-bounce data structures |
| 02 | `02_spectral_dispersion_sellmeier.md` | Sellmeier F1/F2, Cauchy dispersion, Abbe Vd, hero-wavelength sampling, CIE 1931 XYZ |
| 03 | `03_bvh_traversal_heatmap.md` | 2-level TLAS/BLAS, ray-AABB slab test in WGSL, traversal cost heat ramp shader |
| 04 | `04_convergence_analytics.md` | Welford online variance, RMSE tracking, 32-bin SPD histogram |
| 05 | `05_zemax_format_spec.md` | Full .zmx ASCII specification, Cooke Triplet & Double Gauss prescriptions, Rust parser blueprint |
| 06 | `06_vectorcraft_inspired_architecture.md` | Rust workspace layout, design tokens, egui dockable panels, MCP agent integration |
| 07 | `07_deep_dive_rea_evidence.md` | Microscopic AST + semantic evidence across all four reference codebases |
| 08 | `08_optical_aberrations_and_paraxial_math.md` | Paraxial y-u matrices, cardinal points, Seidel aberrations, spot diagrams, ray fans |
| 09 | `09_complete_file_inventory_and_architectural_trace.md` | File-by-file dependency trace across all four repositories |

#### Added — Glass Catalog Database

- **`research/glass_catalogs/optical_glasses.json`** — 646 real-world optical glasses from Schott,
  Ohara, CDGM, and Hoya with verified Sellmeier coefficients and Abbe numbers (nd, Vd).

#### Added — REA Evidence Bundles

- **`rea_evidence_ray_optics.json`** (110 MB) — full static analysis evidence from `ray-optics`.
- **`rea_evidence_three_gpu_pathtracer.json`** (90 MB) — full static analysis evidence from
  `three-gpu-pathtracer`.

#### Added — Project Foundation

- `README.md` — comprehensive project overview, inspiration table, RE library index, workspace
  blueprint, and functional specification.
- `CHANGELOG.md` — this file (Keep a Changelog format).
- `TODO.md` — exhaustive phased implementation task list from workspace scaffold to open-source release.
- `.gitignore` — excludes `research/inspirations/` (large clones) and standard Rust/IDE artifacts.
- `research/` directory structure — `inspirations/`, `reverse_engineering/`, `glass_catalogs/`.

---

## [0.0.0] — 2026-10-15

### Added

- Initial repository created with `README.md`, `LICENSE` (MIT), and `.gitignore`.

---

[Unreleased]: https://github.com/AlexCri5an/optics-lab/compare/v0.1.0-research...HEAD
[0.1.0-research]: https://github.com/AlexCri5an/optics-lab/releases/tag/v0.1.0-research
[0.0.0]: https://github.com/AlexCri5an/optics-lab/releases/tag/v0.0.0
