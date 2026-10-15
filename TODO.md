# OpticsLab — Master Implementation Plan & Task List

> **Status:** Phase 1 ✅ complete. Phase 2.1–2.3, 2.8, 2.9 ✅ complete. Phase 3.1–3.2, 3.4 ✅ complete. Phase 4.1–4.3 ✅ complete. Slice A UI Step A1 & Step A2 (Theme, CAD icons, 2D canvas, translucent prism, Fresnel branches, legend bar, ray inspector, controls, edge states, native app, local web build via trunk) ✅ complete.
>
> Legend: `[ ]` = todo · `[x]` = done · `[~]` = in progress · `[-]` = deferred

---

## Phase 0 — Foundation & Research ✅ COMPLETE

- [x] Create repository, `README.md`, `LICENSE` (MIT), `.gitignore`
- [x] Clone `ray-optics`, `three-gpu-pathtracer`, `optiland`, `vectorcraft`, `rea` into `research/inspirations/`
- [x] Add `research/inspirations/` to `.gitignore`
- [x] Create `research/reverse_engineering/` and `research/glass_catalogs/` directories
- [x] RE Doc 01: Single-Ray Inspector — Snell's Law, Fresnel, TIR, per-bounce data structures
- [x] RE Doc 02: Spectral Dispersion & Sellmeier — Cauchy, Abbe Vd, hero-wavelength, CIE 1931 XYZ
- [x] RE Doc 03: BVH Traversal Heatmap — TLAS/BLAS, ray-AABB slab intersection, cost heatmap WGSL
- [x] RE Doc 04: Convergence & Analytics — Welford variance, RMSE, SPD histogram
- [x] RE Doc 05: Zemax .zmx Format — Full spec, Cooke Triplet, Double Gauss, Rust parser blueprint
- [x] RE Doc 06: VectorCraft Architecture — Rust workspace, design tokens, dockable panels, MCP
- [x] RE Doc 07: Deep-Dive REA Evidence — AST & semantic trace across all four codebases
- [x] RE Doc 08: Aberrations & Paraxial Math — y-u matrices, spot diagram, ray fans, Seidel
- [x] RE Doc 09: Exhaustive File Inventory — file-by-file dependency trace for all 4 repos
- [x] Glass catalog — 646 optical glasses (Schott, Ohara, CDGM, Hoya) with Sellmeier coefficients
- [x] REA evidence bundles — `rea_evidence_ray_optics.json` (105 MB), `rea_evidence_three_gpu_pathtracer.json` (85 MB) in `.gitignore`
- [x] Write `CHANGELOG.md` (Keep a Changelog format)
- [x] Write `TODO.md` (this file)

---

## Phase 1 — Rust Workspace Scaffold ✅ COMPLETE

> Goal: Empty but compiling Rust workspace. CI green. No logic yet.

### 1.1 Workspace Root
- [x] Create `Cargo.toml` (workspace root, `resolver = "3"`, `edition = "2024"`)
- [x] Add `[workspace.lints]` — deny `unsafe_code`, deny `unwrap_used`, deny `expect_used`
- [x] Add `.cargo/config.toml` — set linker to `lld` on Linux CI
- [x] Create `rust-toolchain.toml` pinning stable Rust version (`1.87.0`)
- [x] Create `.github/workflows/ci.yml`:
  - `cargo check --all`
  - `cargo clippy --all -- -D warnings`
  - `cargo test --all`
  - `cargo fmt --check`
  - WASM build check: `cargo build --target wasm32-unknown-unknown -p optics-web`

### 1.2 Crate Scaffolding (empty `lib.rs` stubs only)
- [x] `crates/optics-core/` — `Cargo.toml`, `src/lib.rs`
- [x] `crates/optics-spectral/` — `Cargo.toml`, `src/lib.rs`
- [x] `crates/optics-glass/` — `Cargo.toml`, `src/lib.rs`
- [x] `crates/optics-bvh/` — `Cargo.toml`, `src/lib.rs`
- [x] `crates/optics-zemax/` — `Cargo.toml`, `src/lib.rs`
- [x] `crates/optics-render/` — `Cargo.toml`, `src/lib.rs`
- [x] `crates/optics-ui/` — `Cargo.toml`, `src/lib.rs`

### 1.3 App Scaffolding
- [x] `apps/optics-lab/` — native desktop binary (`eframe` main entry)
- [x] `apps/optics-cli/` — headless binary (no GPU, pure diagnostic runner)
- [x] `apps/optics-web/` — `wasm32-unknown-unknown` target, `index.html`

### 1.4 Dependency Declarations
- [x] `wgpu = "25"` in `optics-render` (matches `eframe = "0.32.3"`)
- [x] `egui = "0.32"` + `eframe = "0.32.3"` in `optics-ui` and `apps/optics-lab`
- [x] `nalgebra = "0.33"` in `optics-core`
- [x] `serde = { version = "1", features = ["derive"] }` workspace-wide
- [x] `serde_json = "1"` in `optics-glass`, `optics-zemax`
- [x] `thiserror = "2"` workspace-wide for structured error types
- [x] `bytemuck = "1"` in `optics-render` (GPU buffer casting)
- [x] `half = "2"` in `optics-render` (f16 texture packing)

---

## Phase 2 — `optics-core`: Ray Primitives & 3D Math

> Source: RE Docs 01, 08

### 2.1 Core Types
- [x] `Ray { origin: Vec3, direction: Vec3, wavelength_nm: f64, energy: f64 }`
- [x] `Vec3` type alias for `nalgebra::Vector3<f64>`
- [x] `HitRecord { t: f64, point: Vec3, normal: Vec3, is_front_face: bool, surface_index: usize }`
- [x] `RayBounce { surface_index, point, normal, incident_dir, reflected_dir, refracted_dir, n1, n2, incident_angle_rad, refracted_angle_rad, rs, rp, r_total, t_total, event, wavelength_nm }` (per-bounce inspector data)
- [x] `OpticsError` enum (thiserror) — `TotalInternalReflection`, `NumericalSingularity`, `InvalidSurface`, `ParseError(String)`

### 2.2 Snell's Law (3D Vector Form)
- [x] `fn snell_refract(incident: Vec3, normal: Vec3, n1: f64, n2: f64) -> Result<Vec3, OpticsError>`
  - Standard numerically stable vector form with TIR detection and flipped-normal robustness
- [x] `fn reflect(incident: Vec3, normal: Vec3) -> Result<Vec3, OpticsError>`
  - Implements: `d - 2 * dot(d, n) * n` with flipped-normal robustness

### 2.3 Fresnel Coefficients
- [x] `fn fresnel_dielectric(cos_theta_i: f64, n1: f64, n2: f64) -> (f64, f64)` — returns exact `(Rs, Rp)` power reflectances
- [x] `fn fresnel_unpolarized(cos_theta_i: f64, n1: f64, n2: f64) -> (f64, f64)` — returns `(R_total, T_total)`
- [x] `fn schlick_approx(cos_theta: f64, n1: f64, n2: f64) -> f64` — fast unpolarized approximation

### 2.4 Conic Surface Geometry (Citardauque Method)
- [ ] `struct ConicSurface { radius_mm: f64, conic_constant: f64, semi_diam_mm: f64 }`
- [ ] `fn intersect_conic(ray: &Ray, surface: &ConicSurface) -> Result<f64, OpticsError>` — Citardauque solver
- [ ] `fn conic_normal(hit_point: Vec3, surface: &ConicSurface) -> Vec3` — analytical normal
- [ ] `fn conic_sag(r: f64, c: f64, k: f64) -> f64` — sag equation z(r)

### 2.5 Optical Surface & Prescription Types
- [ ] `struct OpticalSurface { surface_index, radius_mm, conic_constant, thickness_mm, clear_aperture_radius_mm, glass, is_aperture_stop }`
- [ ] `struct OpticalPrescription { name, entrance_pupil_diameter_mm, surfaces }`
- [ ] Impl `Default` for both (flat plane, no glass)

### 2.6 Paraxial Ray Tracer
- [ ] `fn trace_paraxial_marginal(prescription: &OpticalPrescription) -> ParaxialRay`
- [ ] `fn trace_paraxial_chief(prescription: &OpticalPrescription, field_angle_deg: f64) -> ParaxialRay`
- [ ] `struct ParaxialProperties { efl_mm, bfl_mm, f_number, entrance_pupil_pos_mm, exit_pupil_pos_mm }`
- [ ] `fn calculate_paraxial_properties(prescription: &OpticalPrescription) -> ParaxialProperties`

### 2.7 Tests (unit)
- [x] Test Snell's law with N-BK7 at 587.56 nm (nd=1.5168) — verify refraction angles at 30° and 45°
- [x] Test Snell reversibility: refract then reverse refract restores original direction
- [x] Test Snell geometry: unit length, coplanarity with incident/normal, normal incidence unchanged
- [x] Test TIR at critical angle for N-BK7/air interface (asin(1/n): sub-critical succeeds, super-critical fails)
- [x] Test reflect: angle_i = angle_r, double reflection restores original direction
- [x] Test normal orientation robustness: flipped normals (dot(i, n) > 0) produce identical results
- [x] Test Fresnel at normal incidence (Rs = Rp = ((n1-n2)/(n1+n2))^2)
- [x] Test Fresnel at Brewster's angle (Rp = 0 at atan(n2/n1))
- [x] Test Fresnel grazing incidence (Rs, Rp -> 1.0 as cos_i -> 0)
- [x] Test Fresnel energy conservation (R_total + T_total == 1.0) and TIR ((1.0, 1.0))
- [x] Test Fresnel Stokes symmetry between conjugate Snell angles (n1 -> n2 vs n2 -> n1)
- [x] Test Schlick approximation against exact unpolarized Fresnel (exact at normal/grazing, stated tolerance)
- [ ] Test conic sag: sphere k=0, parabola k=-1 at r=0 → sag=0
- [ ] Test Citardauque solver against analytical root for sphere
- [ ] Test paraxial EFL for a single thin lens (1/f = (n-1)(1/R1 - 1/R2))
- [ ] Test Cooke Triplet paraxial EFL = ~50 mm

### 2.8 Non-Sequential Single-Ray Path Tracer (Slice A Milestone)
- [x] `trait RefractiveIndex { fn n(&self, wavelength_nm: f64) -> f64 }` in `optics-core` (zero dependency on `optics-glass`)
- [x] Triangle mesh geometry with Möller–Trumbore ray-triangle intersection
- [x] `struct Scene { triangles: Vec<Triangle>, media: Vec<Box<dyn RefractiveIndex>> }`
- [x] `enum RayBranchPolicy { FollowTransmission, FollowReflection, Stochastic }`
- [x] `fn trace_path(scene: &Scene, ray: &Ray, policy: RayBranchPolicy, max_bounces: usize) -> PathRecord`
- [x] Equilateral prism test: incident D65 white ray at 45° decomposes into spectral dispersion fan (C=656.27 nm, d=587.56 nm, F=486.13 nm) via Sellmeier dispersion

### 2.9 Spectral Fan & Headless SVG (Slice A Milestone)
- [x] `fn spectral_fan(scene: &Scene, ray_template: &Ray, wavelengths_nm: &[f64], policy: RayBranchPolicy, max_bounces: usize) -> Vec<(f64, PathRecord)>`
- [x] `optics-cli prism --glass N-BK7 --apex 60 --incidence 45 --samples 32 --svg out.svg` draws the prism outline and colored rays (via `display_color_for_wavelength`) as a 2D SVG in the plane of incidence, plus a text table on stdout

### 2.10 Sequential Real-Ray Tracer (Slice B Milestone)
- [ ] `fn trace_sequential(prescription: &ResolvedPrescription, ray: &Ray) -> Result<SequentialPathRecord, OpticsError>`

- [x] Tests: deviation decreases monotonically with wavelength over 380–780 nm; deviation at F, d, C lines equals the analytic two-surface Snell value computed independently inside the test (tolerance 1e-9)

---

## Phase 3 — `optics-spectral`: Dispersion & Color

> Source: RE Docs 02

### 3.1 Sellmeier Dispersion
- [x] `fn sellmeier_formula_1(wavelength_um: f64, b: [f64;3], c: [f64;3]) -> f64` — refractive index n(λ) (takes μm internally)
- [x] `fn sellmeier_formula_2(wavelength_um: f64, a: [f64;3], b: [f64;3]) -> f64`
- [x] `fn cauchy_dispersion(wavelength_um: f64, a: f64, b: f64) -> f64`

### 3.2 Abbe Number & Standard Spectral Lines
- [x] `fn abbe_vd(n_d: f64, n_f: f64, n_c: f64) -> f64` — Vd = (nd - 1) / (nF - nC)
- [x] Standard Fraunhofer lines:
  - Helium yellow d: `LAMBDA_D_NM = 587.56` (0.58756 μm)
  - Hydrogen blue F: `LAMBDA_F_NM = 486.13` (0.48613 μm)
  - Hydrogen red C: `LAMBDA_C_NM = 656.27` (0.65627 μm)

### 3.3 Hero-Wavelength Spectral Sampling
- [ ] `struct HeroSpectrum { wavelengths_nm: [f64; N], weights: [f64; N] }` (N=4 default)
- [ ] `fn hero_sample_wavelengths(primary_nm: f64, n_wavelengths: usize) -> HeroSpectrum`
  - Uniform stratified sampling across 380–780 nm band with primary hero wavelength

### 3.4 CIE 1931 XYZ Color Matching & Piecewise sRGB
- [x] Embed CIE 1931 2° observer data (5 nm steps, 380–780 nm, official CVRL source cited)
- [x] Embed standard CIE Illuminant D65 spectral power distribution table (5 nm steps, 380–780 nm, ISO/CIE 11664-2:2014 cited)
- [x] `fn wavelength_to_xyz(wavelength_nm: f64, radiance: f64) -> [f64; 3]`
- [x] `fn xyz_to_srgb(xyz: [f64; 3]) -> [f32; 3]` — applying D65 white point adaptation and piecewise IEC 61966-2-1 transfer curve (linear slope 12.92 below 0.0031308, 1.055 * u^(1/2.4) - 0.055 above, continuous at junction)
- [x] `fn spectrum_to_srgb(spd: &[(f64, f64)]) -> [f32; 3]` — integrate SPD to sRGB
- [x] `fn display_color_for_wavelength(wavelength_nm: f64) -> [u8; 3]` — converts monochromatic line to sRGB normalized to max channel = 1 (display only)
- [x] Note: Equal-energy spectrum (Illuminant E) is NOT white in sRGB; D65 SPD produces linear sRGB ≈ (1.0, 1.0, 1.0) within 1%

### 3.5 Tests
- [x] N-BK7 Sellmeier: n(587.56 nm) = 1.5168 ± 0.0002
- [x] Fused Silica: n(587.56 nm) = 1.4585
- [x] Abbe Vd for N-BK7 = 64.17 ± 0.1
- [x] D65 SPD integrates to normalized linear sRGB ≈ (1.0, 1.0, 1.0) within 1% tolerance
- [x] Y of D65 normalizes to 1.0
- [x] Monochromatic 550 nm is green-dominant and 700 nm is red-dominant
- [x] sRGB transfer curve linearity (slope 12.92), exponent 2.4, and junction continuity
- [x] Equal-energy spectrum is NOT (1, 1, 1) in sRGB (differs by >15% on red)
- [ ] Hero-wavelength output has correct centroid at primary wavelength (Slice C)

---

## Phase 4 — `optics-glass`: Glass Catalog Database

> Source: RE Doc 02, `research/glass_catalogs/optical_glasses.json`

### 4.1 Glass Data Model
- [x] `struct GlassMaterial { name: String, catalog: GlassCatalog, nd: f64, vd: f64, dispersion: DispersionModel }`
- [x] `enum GlassCatalog { Schott, Ohara, CDGM, Hoya, Custom }`
- [x] `enum DispersionModel { Sellmeier1 { b: [f64;3], c: [f64;3] }, Sellmeier2 { .. }, Cauchy { a, b }, Air }`
- [x] `impl GlassMaterial { fn refractive_index(&self, wavelength_nm: f64) -> f64 }` — public API in nm, converts to μm internally

### 4.2 Catalog Loading (Minimal Slice A implemented)
- [x] Embed `optical_glasses.json` in `crates/optics-glass/data/` via `include_bytes!`
- [x] `struct GlassCatalogDb { glasses_by_name: HashMap<String, GlassMaterial> }`
- [x] `fn load_bundled() -> Result<GlassCatalogDb, OpticsError>` — parse bundled JSON (all 646 glasses)
- [x] `fn lookup(name: &str) -> Option<&GlassMaterial>` — case-insensitive lookup with aliases
- [ ] `fn search_by_nd_vd(nd_range: (f64,f64), vd_range: (f64,f64)) -> Vec<&GlassMaterial>` — Abbe diagram filtering (Slice D)

### 4.3 Preset Glasses (verified against catalog entries, not independently hardcoded)
- [x] `GlassMaterial::n_bk7()` — Schott N-BK7 (nd=1.5168, Vd=64.17 from catalog)
- [x] `GlassMaterial::sf11()` — Schott SF11 (nd=1.7847, Vd=25.76 from catalog)
- [x] `GlassMaterial::f2()` — Schott F2 (nd=1.6200, Vd=36.37 corrected from catalog)
- [x] `GlassMaterial::fused_silica()` — UV-grade SiO2 / Lithosil-Q (nd=1.4585, Vd=67.83 from catalog)
- [x] `GlassMaterial::air()` — n=1.0000 constant at all wavelengths

### 4.4 Tests
- [x] `GlassCatalogDb` loads all 646 entries without panic
- [x] N-BK7 lookup returns n(587.56 nm) = 1.5168 ± 0.001
- [x] Test every preset (N-BK7, SF11, F2, Fused Silica) matches catalog entry for nd, Vd, and n(lambda) at 486.13, 587.56, 656.27 nm within 1e-5
- [x] Air always returns n=1.0000 at all wavelengths
- [ ] Search by nd ∈ [1.5, 1.52] returns non-empty list containing N-BK7 (Slice D)

---

## Phase 5 — `optics-zemax`: File Format Parser

> Source: RE Doc 05, `optiland/fileio/zemax/`

### 5.1 Zemax .zmx Parser
- [ ] `fn parse_zmx(content: &str) -> Result<ZemaxOpticalSystem, ZemaxParseError>`
- [ ] Handle all operands: `NAME`, `ENPD`, `FNUM`, `WAVL`, `PWAV`, `FTYP`, `YFLD`, `SURF`, `TYPE`, `CURV`, `DISZ`, `GLAS`, `DIAM`, `CONI`, `STOP`
- [ ] Convert `CURV c` → `radius = if |c| < 1e-12 { f64::INFINITY } else { 1.0/c }`
- [ ] `enum SurfaceType { Standard, EvenAsphere, Toroidal, ZernikeSag }` (extensible)
- [ ] `struct ZemaxParseError` (thiserror) with line number and operand context

### 5.2 Zemax .zmx Serializer (Writer)
- [ ] `fn write_zmx(system: &ZemaxOpticalSystem) -> String` — round-trip serialization
- [ ] Round-trip test: parse → write → parse gives identical result

### 5.3 Lens Presets (Bundled)
- [ ] Cooke Triplet F/4.5 (7 surfaces, SK16/F2/SK16) — as `.zmx` string constant
- [ ] Double Gauss F/2.0 (10 surfaces) — as `.zmx` string constant
- [ ] Single Plano-Convex N-BK7 f=100 mm — as `.zmx` string constant

### 5.4 CODE V .seq Parser [-] DEFERRED
> Deferred: focus on complete Zemax .zmx pipeline first.
- [-] Minimal `fn parse_seq(content: &str) -> Result<OpticalPrescription, CodeVError>`
- [-] Handles `WL`, `IMS`, `OBB`, `INS`, `AIR`, `REFL` tokens

### 5.5 Tests
- [ ] Parse Cooke Triplet preset: 7 surfaces, correct radius/thickness/glass values
- [ ] Parse Double Gauss: 10 surfaces, STOP surface flagged correctly
- [ ] Round-trip Cooke Triplet: write then re-parse gives identical prescription
- [ ] `CURV 0.0` → `radius = f64::INFINITY`
- [ ] `GLAS AIR` → `material_name = "AIR"`

---

## Phase 6 — `optics-bvh`: Acceleration Structure

> Source: RE Doc 03, `three-gpu-pathtracer/src/shader/bvh/`

### 6.1 AABB & BVH Node
- [ ] `struct Aabb { min: Vec3, max: Vec3 }` with `fn grow(&mut self, pt: Vec3)`, `fn surface_area() -> f64`
- [ ] `fn ray_aabb_slab_test(ray: &Ray, aabb: &Aabb) -> Option<(f64, f64)>` — exact slab method with safe NaN handling
- [ ] `struct BvhNode { aabb: Aabb, left_or_first: u32, count: u32, is_leaf: bool }`

### 6.2 CPU BVH Builder (SAH)
- [ ] `fn build_bvh(primitives: &[Primitive]) -> Vec<BvhNode>` — Surface Area Heuristic
- [ ] Binned SAH with 16 bins per axis
- [ ] `fn find_best_split(node: &BvhNode, primitives: &[Primitive]) -> SplitDecision`

### 6.3 2-Level BVH (TLAS / BLAS)
- [ ] BLAS: per-mesh BVH over triangles
- [ ] TLAS: BVH over BLAS instances (for multi-object scenes)
- [ ] `struct BvhScene { tlas: Vec<BvhNode>, blas: Vec<Vec<BvhNode>>, triangles: Vec<Triangle> }`

### 6.4 BVH GPU Buffer Layout (16-Byte Aligned)
- [ ] 16-byte aligned flat GPU buffer pack:
  - `struct GpuBvhNode { aabb_min_left: [f32; 4], aabb_max_count: [f32; 4] }` where $w$-components pack `left_or_first: u32` and `count: u32` (or explicitly padded 32-byte struct with 16-byte alignment)
- [ ] `fn pack_bvh_to_gpu_buffer(nodes: &[BvhNode]) -> Vec<u8>` — `bytemuck`-safe
- [ ] Triangle buffer: `[v0: f32x4, v1: f32x4, v2: f32x4, normal: f32x4]` (16-byte aligned)
- [ ] Unit test: verify `std::mem::size_of::<GpuBvhNode>() == 32` and `std::mem::align_of::<GpuBvhNode>() == 16`

### 6.5 Traversal Cost Counter (CPU reference)
- [ ] `fn trace_bvh_cost(ray: &Ray, bvh: &BvhScene) -> u32` — returns intersection test count

### 6.6 Tests
- [ ] Ray-AABB: miss past, miss before, miss off-axis, hit returns correct t range
- [ ] Single-triangle BVH: 1 node, 1 triangle, hit returns t ≈ distance
- [ ] SAH split produces valid left/right partition (no empty sides)
- [ ] CPU traversal cost > 0 for hit ray, = 0 for early-exit miss ray

---

## Phase 7 — `optics-render`: wgpu Path Tracing Pipeline

> Source: RE Docs 03, 06, 07 + `three-gpu-pathtracer/src/webgpu/`

### 7.1 GPU Context Setup
- [ ] `struct GpuContext { instance, adapter, device, queue }` — wgpu instance creation
- [ ] `fn create_gpu_context(window: &Window) -> GpuContext` — with Surface for desktop
- [ ] `fn create_headless_context() -> GpuContext` — for CLI / tests
- [ ] Handle WASM target: `web_sys::HtmlCanvasElement` surface

### 7.2 Render Targets
- [ ] `struct RenderTarget { texture, view, format: TextureFormat::Rgba16Float }` — HDR accumulation buffer
- [ ] `struct ToneMapTarget { texture, view, format: TextureFormat::Bgra8UnormSrgb }` — display buffer
- [ ] `fn resize_render_targets(ctx: &GpuContext, width: u32, height: u32)`

### 7.3 BVH Upload Pass
- [ ] `struct BvhBuffer { node_buf: Buffer, triangle_buf: Buffer, material_buf: Buffer }`
- [ ] `fn upload_bvh(ctx: &GpuContext, scene: &BvhScene) -> BvhBuffer`

### 7.4 Path Tracing Compute Shader (WGSL)
- [ ] `src/shaders/path_tracer.wgsl` — main compute shader:
  - [ ] PCG32 pseudo-random generator
  - [ ] Ray generation from thin-lens camera (aperture, DOF)
  - [ ] BVH stack traversal (iterative, 32-deep stack)
  - [ ] Lambert BRDF for diffuse surfaces
  - [ ] GGX microfacet BSDF (from RE Doc 07) for glossy/metal
  - [ ] Fresnel dielectric (Schlick) for transparent/glass
  - [ ] Sellmeier dispersion lookup via storage buffer
  - [ ] Russian Roulette path termination at depth ≥ 3
  - [ ] Accumulate sample into `f32x4` HDR buffer (additive blend)

### 7.5 Spectral Dispersion Compute Pass
- [ ] `src/shaders/spectral_dispersion.wgsl`:
  - [ ] Hero-wavelength sampling across 380–780 nm (4 wavelengths per ray)
  - [ ] Sellmeier lookup table in storage buffer (per glass, 64 wavelength entries)
  - [ ] XYZ → sRGB conversion kernel

### 7.6 BVH Traversal Cost Heatmap Shader
- [ ] `src/shaders/bvh_heatmap.wgsl`:
  - [ ] Per-pixel traversal cost counter
  - [ ] Heat ramp: `cost=0` → `#0a1628` (dark navy), `cost=max` → `#ff4422` (red-orange)
  - [ ] Configurable max_cost uniform (auto-ranging via readback)

### 7.7 Convergence Analytics
- [ ] `struct ConvergenceTracker { sample_count: u64, welford_mean: f32, welford_m2: f32 }`
- [ ] Welford online update on CPU side per frame
- [ ] RMSE readback via `wgpu::Buffer::MAP_READ` staging buffer

### 7.8 Tone Mapping Pass
- [ ] `src/shaders/tonemapping.wgsl` — ACES filmic tone mapping operator
- [ ] Gamma correction: linear → sRGB using piecewise IEC 61966-2-1 transfer curve (linear segment below 0.0031308, not simple power 2.2)

### 7.9 Camera Models
- [ ] `struct ThinLensCamera { position: Vec3, look_at: Vec3, up: Vec3, fov_deg: f64, aperture_mm: f64, focus_dist_mm: f64 }`
- [ ] `fn generate_ray(camera: &ThinLensCamera, px: f32, py: f32, sample: (f32,f32)) -> Ray`
- [ ] `struct OrthographicCamera` for paraxial layout view

### 7.10 Render Pipeline Orchestration
- [ ] `struct PathTracingRenderer { context, bvh_buffer, compute_pipeline, bind_groups, accumulation_buffer, sample_count }`
- [ ] `fn render_frame(&mut self)` — dispatch compute → tonemapping → present
- [ ] `fn reset_accumulation(&mut self)` — called on scene change or camera move
- [ ] `fn get_sample_count(&self) -> u64`

---

## Phase 8 — Optical Analysis Suite

> Source: RE Doc 08, `optiland/analysis/`

### 8.1 Geometric Spot Diagram
- [ ] `fn trace_hexapolar_grid(prescription: &OpticalPrescription, field_angle_deg: f64, num_rings: usize) -> Vec<[f64;2]>`
- [ ] `fn spot_diagram_metrics(points: &[[f64;2]]) -> SpotMetrics`
- [ ] `struct SpotMetrics { centroid_x, centroid_y, rms_radius_um, geo_radius_um, airy_radius_um, is_diffraction_limited }`

### 8.2 Transverse Ray Aberration Fans
- [ ] `fn trace_ray_fan(prescription: &OpticalPrescription, field_angle_deg: f64, num_points: usize) -> RayFanData`
- [ ] `struct RayFanData { tangential: Vec<(f64,f64)>, sagittal: Vec<(f64,f64)> }` — `(pupil_coord, aberration_um)`

### 8.3 MTF (Modulation Transfer Function) [-] DEFERRED
> Deferred: post-Slice A analytical enhancement.
- [-] `fn compute_geometric_mtf(spot_diagram: &SpotDiagram, spatial_freqs_lp_mm: &[f64]) -> Vec<f64>`

### 8.4 Field Curvature & Distortion [-] DEFERRED
> Deferred: post-Slice A analytical enhancement.
- [-] `fn field_curvature(prescription: &OpticalPrescription, field_angles: &[f64]) -> FieldCurvatureData`
- [-] `fn distortion(prescription: &OpticalPrescription, field_angles: &[f64]) -> Vec<f64>` — percentage distortion

### 8.5 Seidel Aberration Coefficients [-] DEFERRED
> Deferred: post-Slice A analytical enhancement.
- [-] `fn compute_seidel_coefficients(prescription: &OpticalPrescription) -> SeidelCoefficients`
- [-] `struct SeidelCoefficients { S1_spherical, S2_coma, S3_astigmatism, S4_field_curvature, S5_distortion, C1_longitudinal_chromatic, C2_transverse_chromatic }`

---

## Phase 9 — `optics-ui`: Design System & Panels

> Source: RE Doc 06, `vectorcraft/crates/ui-egui/`

### 9.1 Design Tokens (Theme System)
- [x] `struct OpticsTokens { panel_bg, canvas_bg, text_primary, text_dim, border, accent, amber, ... }` (egui `Color32`)
- [ ] `enum ThemeBrightness { Dark, MediumDark, MediumLight, Light }` (deferred to Slice D)
- [ ] `fn tokens_for_brightness(b: ThemeBrightness) -> OpticsTokens` — 4 complete palettes (deferred to Slice D)
- [x] Default: `MediumDark` — dark slate `#1a1f2e`, canvas `#11141a`, vivid cyan accent `#00d4f5`, amber highlights `#f5a623` (WCAG AA compliant)
- [x] `fn apply_tokens(ctx: &egui::Context, tokens: &OpticsTokens)` — push `egui::Style`

### 9.2 Dockable Panel System
- [ ] `struct DockState { layout: DockLayout, active_panel: Option<PanelId> }`
- [ ] `enum PanelId { SingleRayInspector, LensPrescription, GlassCatalog, Convergence, SpectralHistogram, BvhHeatmap }`
- [ ] `struct DockLayout { left_width_frac: f32, right_width_frac: f32, bottom_height_frac: f32 }`
- [ ] Drag-and-drop panel re-ordering (egui `SidePanel` + `TopBottomPanel`)
- [ ] Collapsible panels with animated open/close transition

### 9.3 Central Canvas Widget (Slice A: 2D Canvas via egui Painter)
- [x] `struct CanvasState` / `ViewTransform` (world <-> screen, pan, cursor-anchored zoom, margin auto-fit)
- [x] 2D scene canvas (prism, light, rays) drawn directly with egui's `Painter` (zero wgpu texture, no 3D viewer in Slice A)
- [x] Pan: middle-mouse drag or Alt+drag; Zoom: scroll wheel (smooth zoom centered on cursor)
- [x] Click-to-select a ray by nearest-segment hit test; hover to highlight trajectory; Esc to deselect
- [ ] Diagnostic overlay selector: Normal 2D / Spectral view
- [ ] [Slice C] Blit GPU texture from wgpu path tracer to egui `Image` with 3D camera and BVH heatmap overlay

### 9.4 Left Tool Palette
- [x] Select tool (S)
- [x] Ray Inspector Probe tool (I) — click-to-inspect single ray (active by default; planned tools disabled with tooltip "Coming later")
- [ ] Lens Surface tool (L) — add/move lens elements in 2D paraxial view
- [ ] Light Source tool (E) — add point/area/directional emitters
- [ ] Measure tool (M) — distance, angle, focal point probe
- [ ] Tool palette icons — SVG inline or egui `TextureId` bitmaps

### 9.5 Right Inspector Panel Tabs
- [x] **Tab: Single-Ray Inspector (Slice A)**
  - [x] Per-bounce table from `RayBounce`: Surface #, Snell angle θi/θt (°), n1/n2, Rs, Rp, R total, T total, TIR flag
  - [x] Wavelength readout, color swatch (via `display_color_for_wavelength`), and total angular deviation
  - [ ] [Slice C] 3D trajectory mini-viewer (embedded wgpu sub-view) and energy pie chart
- [ ] **Tab: Lens Prescription Editor**
  - [ ] Editable data table: Surface, Type, Radius (mm), Thickness (mm), Glass, Semi-Diam (mm), Stop
  - [ ] Add/remove surface row buttons
  - [ ] Live re-trace on any cell edit
  - [ ] "Load .zmx file" button → file dialog → parse → update table
  - [ ] "Export .zmx" button → save dialog → write_zmx
  - [ ] Lens preset selector (dropdown): Cooke Triplet, Double Gauss, Plano-Convex, Custom
- [ ] **Tab: Glass Catalog Browser**
  - [ ] Search field (filter by name substring)
  - [ ] Abbe diagram (Vd on X, nd on Y) — egui `Plot` with colored dots
  - [ ] Scrollable glass list with nd, Vd, catalog columns
  - [ ] Click glass → show full Sellmeier coefficients & n(λ) dispersion curve

### 9.6 Bottom Analytics Dock Tabs
- [ ] **Tab: Convergence Monitor**
  - [ ] Real-time RMSE line chart (egui `Plot`, scrolling last 500 samples)
  - [ ] Mean variance bar, sample count display, estimated convergence ETA
- [ ] **Tab: Spectral Power Distribution**
  - [ ] 32-bin histogram from 380–780 nm for selected pixel/region
  - [ ] Wavelength color coding (380=violet, 450=blue, 520=green, 590=yellow, 650=red)
  - [ ] XYZ → sRGB preview swatch for the selected SPD
- [ ] **Tab: BVH Heatmap Controls**
  - [ ] Toggle heatmap overlay on/off
  - [ ] Max cost slider (auto / manual)
  - [ ] AABB wireframe toggle per BVH level (TLAS / BLAS)
  - [ ] Cost statistics: median, 95th percentile, max

### 9.7 Top Menu Bar
- [ ] **File**: New scene, Open .zmx, Save, Export PNG/EXR
- [ ] **View**: Toggle panels, Reset layout, Fullscreen canvas
- [ ] **Render**: Start/Pause, Reset accumulation, Resolution picker
- [ ] **Diagnostics**: Single-ray inspect mode, BVH heatmap, Spectral view
- [ ] **Help**: Open docs (browser), About dialog, REA research notes link
- [ ] Keyboard shortcut editor (show/hide with `?`)

### 9.8 Widgets Library
- [ ] `fn scrub_field(ui: &mut Ui, label: &str, value: &mut f64, min: f64, max: f64, precision: usize)` — drag-to-edit numeric field
- [ ] `fn dispersion_curve_plot(ui: &mut Ui, glass: &GlassMaterial)` — n(λ) plot 380–780 nm
- [ ] `fn abbe_diagram(ui: &mut Ui, catalog: &GlassCatalogDb, highlighted: Option<&str>)`
- [ ] `fn spot_diagram_widget(ui: &mut Ui, data: &SpotMetrics)`
- [ ] `fn ray_fan_widget(ui: &mut Ui, data: &RayFanData)`

---

## Phase 10 — Native Desktop App (`apps/optics-lab`)

> Source: RE Doc 06 + eframe integration

### 10.1 Main App Struct
- [ ] `struct OpticsLabApp { dock_state, canvas_state, render_state, prescription, glass_db, inspector_ray, analytics }`
- [ ] `impl eframe::App for OpticsLabApp { fn update(&mut self, ctx, frame) }`
- [ ] 60 fps target — render GPU frame, tonemap, blit to egui `Image`
- [ ] Window icon — optical prism SVG rendered to 256×256 PNG

### 10.2 Application State Machine
- [ ] `enum AppMode { Idle, Rendering, Inspecting, EditingLens, BvhDiagnostic }`
- [ ] State transitions on tool selection / keyboard shortcuts
- [ ] Undo/redo stack (VectorCraft-style) — `Vec<AppSnapshot>` with max depth 50

### 10.3 File Dialogs
- [ ] Open `.zmx` — `rfd::AsyncFileDialog` → parse → update prescription → reset render
- [ ] Export PNG — `image` crate to encode BGRA8 readback to PNG
- [ ] Export EXR — `exr` crate to encode RGBA16F accumulation buffer

### 10.4 Keyboard Shortcuts
| Shortcut | Action |
|----------|--------|
| `S` | Select tool |
| `I` | Ray Inspector Probe |
| `L` | Lens Surface tool |
| `E` | Light Source tool |
| `M` | Measure tool |
| `Space` | Pause/Resume render |
| `R` | Reset accumulation |
| `Ctrl+O` | Open .zmx file |
| `Ctrl+S` | Save project |
| `Ctrl+Shift+E` | Export PNG |
| `Ctrl+Z` / `Ctrl+Shift+Z` | Undo / Redo |
| `F11` | Toggle fullscreen canvas |
| `?` | Keyboard shortcut reference |

### 10.5 macOS-Specific Polish [-] DEFERRED
> Deferred until core application UI and rendering stabilize.
- [-] Native menu bar (via `eframe` native menu)
- [-] Retina display: `scale_factor` aware rendering
- [-] `Info.plist` with bundle ID `com.opticslab.app`, version, icon

---

## Phase 11 — CLI App (`apps/optics-cli`)

> For benchmarking, CI validation, and headless batch rendering

### 11.1 Commands
- [ ] `optics-cli render --zmx <file> --output <png> --spp <N>` — headless render
- [ ] `optics-cli analyze --zmx <file>` — print paraxial properties + Seidel coefficients to stdout
- [ ] `optics-cli glass --name <name>` — print glass n(λ) table + Abbe diagram text
- [ ] `optics-cli benchmark --scene <scene>` — MRAYS/s throughput benchmark

### 11.2 Output Formats
- [ ] JSON output with `--json` flag for all analysis commands
- [ ] CSV export for spot diagram data (`--csv`)

---

## Phase 12 — WebAssembly App (`apps/optics-web`)

> Browser deployment via egui `wasm32-unknown-unknown` + WebGL/Glow (WebGPU in Slice C)

### 12.1 Local WASM Build (via Trunk)
- [x] Setup `Trunk.toml` configuration for `apps/optics-web`
- [x] `trunk build --release` produces standalone bundle in `dist/` (verified locally: 3.20 MB raw, 1.44 MB gzip)
- [x] `index.html` with WebGL/WASM availability check + loading indicator + error fallback banner
- [x] `dist/` and `.trunk/` in `.gitignore`
- [-] Web worker for heavy BVH build (off main thread) — deferred for Slice A

### 12.2 Browser Compatibility
- [x] WebGL2/WebGL detection with plain-text fallback banner for unsupported browsers
- [ ] [Slice C] WebGPU feature detection and browser compatibility matrix (Chromium 113+, Firefox Nightly, Safari TP)

### 12.3 WASM-Specific Features
- [ ] Drag-and-drop `.zmx` file onto canvas to load
- [ ] Embed sample Cooke Triplet scene as default WASM load
- [-] Share URL: encode scene state as base64 URL param — deferred
- [-] PWA manifest for offline install (`manifest.json`, service worker) — deferred

---

## Phase 13 — MCP Agent Integration [-] DEFERRED

> Deferred: internal stdio MCP server planned post-v0.2. Antigravity IDE already provides workspace context.

### 13.1 Internal MCP Server (opt-in)
- [-] `crates/optics-mcp/` — Model Context Protocol server over stdio
- [-] Tool: `get_prescription` → returns current `OpticalPrescription` as JSON
- [-] Tool: `set_surface` → modify a surface parameter and retrigger render
- [-] Tool: `trace_ray` → fire a specific ray, return per-bounce JSON data
- [-] Tool: `get_spot_diagram` → return SpotMetrics JSON
- [-] Tool: `optimize_surface` → AI-driven lens optimization hook (stub)
- [-] Integration in `.agents/mcp_config.json` for Antigravity agent access

---

## Phase 14 — Polish, Testing & Profiling

### 14.1 Integration Tests
- [ ] End-to-end: load Cooke Triplet `.zmx` → trace 100 SPP → EFL within 0.1% of expected
- [ ] End-to-end: spot diagram RMS radius < 50 μm for Cooke Triplet at 0° field
- [ ] Round-trip: parse → write → parse `.zmx` gives identical prescription
- [ ] WASM build test: headless test via trunk/wasm-bindgen-test

### 14.2 Performance Benchmarks (`apps/optics-cli benchmark`)
> Note: Performance numbers are aspirational targets, not hard release blockers.
- [ ] Aspirational target: ≥ 200 MRAYS/s on Apple M-series (wgpu + Metal)
- [ ] Aspirational target: ≥ 100 MRAYS/s on integrated Intel/AMD GPU (wgpu + Vulkan)
- [ ] Aspirational target: BVH build time ≤ 50 ms for 1M triangles

### 14.3 GPU Profiling
- [ ] Enable `wgpu` timestamp queries for per-pass timing
- [ ] Integrate `puffin` for CPU-side frame profiling
- [ ] Export flamegraph with `cargo flamegraph`

### 14.4 Visual Regression Tests
- [ ] Golden-image test suite: render Cooke Triplet, diff against reference PNG (< 1% RMSE)
- [ ] BVH heatmap golden image: blue-to-red gradient validates shader

---

## Phase 15 — Documentation & Community

### 15.1 Code Documentation
- [ ] `#![deny(missing_docs)]` in all `lib.rs` crates
- [ ] Rustdoc for every `pub` item — including math formula in LaTeX (mdBook compatible)
- [ ] `cargo doc --no-deps --open` renders clean documentation

### 15.2 User Documentation (mdBook)
- [ ] `docs/` mdBook with:
  - [ ] Getting Started (build from source, run native, run WASM)
  - [ ] Optical Theory Reference (Snell, Fresnel, Sellmeier — pulled from RE docs)
  - [ ] Glass Catalog Guide (how to read Abbe diagram, add custom glass)
  - [ ] Zemax Import Guide
  - [ ] Lens Design Tutorial (build a Cooke Triplet from scratch)
  - [ ] Keyboard Shortcut Reference
  - [ ] MCP Agent API Reference

### 15.3 Contributing Guide
- [ ] `CONTRIBUTING.md`:
  - [ ] Code of Conduct
  - [ ] Development setup (rustup, wgpu requirements, node for docs)
  - [ ] PR checklist: tests pass, clippy clean, docs updated
  - [ ] RE methodology note: how to add new reference analysis docs

---

## Phase 16 — Release (starts only when user explicitly requests) [-] DEFERRED

> Deferred: zero publishing, tagging, deployments or release packaging permitted until explicit user instruction.

### Milestone Gate: Feature-Complete + Polish
- [-] All quality gates pass (`cargo fmt`, `cargo clippy -D warnings`, `cargo test`, `cargo build --target wasm32-unknown-unknown`)
- [-] Golden image visual regression suite clean at 1x and 2x DPI
- [-] Complete documentation in mdBook and rustdoc clean with 0 warnings
- [-] Zero unverified claims in reports

### 16.1 Licensing & Third-Party Notices
- [-] Verify redistribution terms and data provenance of glass catalog data (Schott, Ohara, CDGM, Hoya)
- [-] Add `THIRD_PARTY_NOTICES.md` acknowledging algorithms, code, and design ported from reference repos

### 16.2 Packaging & Binaries
- [-] Tag release with pre-built binaries for macOS (arm64), Windows (x64), Linux (x64)
- [-] macOS `.app` / `.dmg` packaging with `.icns` icon via `cargo-packager` or `cargo-bundle`
- [-] Release notes (generated from `CHANGELOG.md`)
- [-] crates.io publish for workspace crates (optics-core, optics-spectral, optics-glass, optics-zemax)
- [-] `brew tap` formula for macOS install

### 16.3 Web & Documentation Deployments
- [-] GitHub Actions workflow that builds with trunk and deploys `dist/` to GitHub Pages
- [-] Deploy mdBook docs to GitHub Pages

---

## Appendix: Dependency Summary

| Crate | Key Dependencies |
|-------|-----------------|
| `optics-core` | `nalgebra`, `serde`, `thiserror` |
| `optics-spectral` | `optics-core`, `serde` |
| `optics-glass` | `optics-core`, `optics-spectral`, `serde_json` |
| `optics-bvh` | `optics-core`, `bytemuck` |
| `optics-zemax` | `optics-core`, `optics-glass`, `thiserror` |
| `optics-render` | `optics-core`, `optics-bvh`, `wgpu`, `bytemuck`, `half` |
| `optics-ui` | `optics-core`, `optics-spectral`, `optics-glass`, `egui`, `eframe` |
| `optics-mcp` | `optics-core`, `optics-render`, `serde_json` |
| `apps/optics-lab` | `optics-ui`, `eframe`, `rfd`, `image`, `exr` |
| `apps/optics-cli` | `optics-core`, `optics-spectral`, `optics-glass`, `optics-zemax`, `clap` |
| `apps/optics-web` | `optics-ui`, `wasm-bindgen`, `web-sys` |

---

## Implementation Slices (Vertical Delivery Plan)

### Slice A — First Interactive Demo (Prism Spectral Dispersion)
> Goal: Prism demo complete and polished; runs natively; builds for web locally.
- [x] Phase 1: Rust workspace scaffold & CI
- [x] Phase 2.1–2.3: Core math (Ray, Snell vector refract, exact Fresnel, TIR)
- [x] Task 2.8: Non-sequential path tracer (triangles, Möller–Trumbore, D65 prism scene)
- [x] Task 2.9: Spectral fan & headless SVG export (`optics-cli prism`)
- [x] Phase 3.1–3.2: Sellmeier formula 1 & Abbe Vd
- [x] Phase 3.4: CIE 1931 XYZ, piecewise IEC 61966-2-1 sRGB, D65 SPD, `display_color_for_wavelength`
- [x] Phase 4.1–4.3: Embedded glass catalog (646 glasses) & verified presets (N-BK7, SF11, F2, Fused Silica, Air)
- [x] Phase 9.1: Design tokens & theme palette (MediumDark, WCAG AA verified)
- [x] Phase 9.3: Central 2D scene canvas (prism, light, rays) via egui `Painter`
- [x] Phase 9.5: Single-Ray Inspector tab (click-to-select ray by nearest segment, per-bounce table from `RayBounce`, angle readouts)
- [x] Phase 12.1: Local WebAssembly build via `trunk` (verified locally; release wasm: 3.20 MB raw / 1.44 MB gzip)
- [x] Step A2: UI visual and interaction polish (glyphs, bounds auto-fit, translucent prism, Fresnel reflection branches, legend, sentence case, chrome fix)

### Slice B — Lens Systems & Sequential Diagnostics
> Goal: Load Zemax Cooke Triplet and Double Gauss prescriptions, trace marginal/chief paraxial rays, and render 2D spot diagrams and ray fans.
- [ ] Phase 2.4–2.6, 2.10: Conic surfaces, Citardauque intersection solver, prescriptions, paraxial tracer, sequential ray tracer
- [ ] Phase 5: Zemax `.zmx` parser, serializer, and bundled lens presets
- [ ] Phase 8.1–8.2: Geometric spot diagrams & transverse ray aberration fans
- [ ] Phase 9.5: Lens prescription editor table with live edit & re-trace
- [ ] Phase 11: `optics-cli analyze` CLI command

### Slice C — Acceleration, GPU Path Tracing & Full Analytics
> Goal: wgpu compute path tracer with 2-level BVH, traversal cost heatmap, and real-time convergence tracking.
- [ ] Phase 3.3: Hero-wavelength spectral sampling (uniform stratified sampling for GPU path tracer)
- [ ] Phase 6: 2-level BVH (TLAS/BLAS), binned SAH builder, 16-byte aligned GPU layout
- [ ] Phase 7: wgpu path tracing compute shaders, hero-wavelength sampling, BVH heatmap, Welford RMSE
- [ ] Phase 9.3 (Slice C): 3D viewport canvas blitting wgpu GPU texture to egui `Image`
- [ ] Phase 9.6: Bottom analytics dock (Convergence monitor, SPD histogram 380–780 nm, heatmap controls)
- [ ] Phase 11: `optics-cli render` and throughput benchmark

### Slice D — Polish, Full Catalog, WASM Release & Open Source
> Goal: Production-grade desktop and browser app with 646 optical glasses, complete documentation, and open source release.
- [ ] Phase 4.2 & 4.4: Full 646 optical glass Abbe diagram search/filter in UI
- [ ] Phase 9.4: Tool palette (Select, Ray Probe, Lens Surface, Emitter, Measure)
- [ ] Phase 9.7: Top menu bar & keyboard shortcuts
- [ ] Phase 10: Native desktop application (`apps/optics-lab`), file dialogs
- [ ] Phase 14: Integration test suite, visual golden tests, profiling
- [ ] Phase 15: mdBook documentation, licensing, `THIRD_PARTY_NOTICES.md`, GitHub release

---

> **Next action:** Slice B, Step B1: Sequential optics core (conic surfaces, prescriptions, paraxial tracer, sequential ray tracer).

