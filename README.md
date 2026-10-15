# OpticsLab

An open-source, GPU-accelerated visual laboratory for optics, spectral dispersion, and path tracing diagnostics.

OpticsLab is a clean-room, open-source alternative to commercial optical engineering suites (*Zemax OpticStudio*, *CODE V*, *LightTools*) and research renderers (*Mitsuba 3*, *pbrt-v4*). Built with pure Rust, `wgpu`, and `egui`, it delivers real-time physical ray tracing, interactive single-ray inspection, and comprehensive optical diagnostics across native desktop and web browsers (WebAssembly/WebGPU).

---

## Interactive Prism Dispersion (Slice A Preview)

OpticsLab features an interactive 2D prism simulation demonstrating real-time chromatic dispersion across physical glass media (N-BK7, SF11, F2, Fused Silica). An incoming D65 standard illuminant white ray refracts according to Sellmeier dispersion formulas, producing a continuous spectral fan sampled across visible wavelengths. Clicking any ray selects it and opens the inspector panel displaying exact per-bounce Fresnel reflectance/transmittance, angles of incidence/refraction, and cumulative throughput.

<!-- Screenshot placeholder: Slice A Prism Dispersion UI with Spectral Fan and Ray Inspector (1200x800) -->
<!-- ![OpticsLab Slice A UI Screenshot](docs/images/slice_a_prism_ui.png) -->

### Running Locally

#### Native Desktop App (macOS, Linux, Windows)
```bash
cargo run -p optics-lab
```

#### WebAssembly (Local Browser via Trunk)
```bash
# Install trunk if not present: brew install trunk  OR  cargo install trunk
cd apps/optics-web
trunk serve
```
Open `http://127.0.0.1:8080` in your web browser.

---

## Inspiration & References

OpticsLab is inspired by a broad landscape of research tools, commercial software, and modern open-source initiatives:
* **Renderers & Research Tracers:** *Mitsuba 3*, *pbrt-v4*, *LuxCoreRender*, *Blender Cycles*, *three-gpu-pathtracer*, and *rayfinder*.
* **Optics & Lens Simulators:** *Ray Optics Simulation (2D)*, *Optiland*, *Raypier*, *DeepLens*, and *Prism Light Simulator*.
* **Industry Optical Suites:** *Zemax OpticStudio*, *CODE V*, *LightTools*, and *Meep (FDTD)*.
* **Rust Architecture Inspiration:** [*VectorCraft*](https://github.com/storytold/vectorcraft) — adopting pure Rust workspace separation, zero-panic policy, tokenized dark theme, and dockable panel UX.
* **Reverse Engineering Methodology:** [*REA (morluto/rea)*](https://github.com/morluto/rea) — structured extraction of optical mathematics, glass catalogs, and diagnostic shaders.

---

## Reverse Engineering & Research Library

All inspirations have been systematically reverse-engineered into technical specification documents:

| Specification Document | Scope & Algorithms Extracted |
|------------------------|------------------------------|
| [**01. Single-Ray Inspector**](research/reverse_engineering/01_single_ray_inspector.md) | Snell's law in 3D vector form, Fresnel dielectric coefficients ($R_s, R_p$), total internal reflection (TIR), ray bounce trajectory data structures, and per-bounce energy loss. |
| [**02. Spectral Dispersion & Sellmeier**](research/reverse_engineering/02_spectral_dispersion_sellmeier.md) | Sellmeier Formula 1 & 2 equations, Cauchy dispersion, Abbe $V_d$ number, real glass catalogs (Schott N-BK7, SF11, F2, Fused Silica), Hero-wavelength sampling (380–700 nm), and CIE 1931 XYZ color matching. |
| [**03. BVH Traversal Cost Heatmap**](research/reverse_engineering/03_bvh_traversal_heatmap.md) | 2-level BVH (TLAS / BLAS), ray-AABB slab intersection in WGSL, traversal cost accumulation, and blue-green-yellow-red diagnostic heat ramp. |
| [**04. Convergence & Analytics**](research/reverse_engineering/04_convergence_analytics.md) | Welford's algorithm for online sample variance, RMSE tracking over time, and 32-bin spectral power distribution (SPD) histograms. |
| [**05. Zemax .zmx Specification**](research/reverse_engineering/05_zemax_format_spec.md) | Complete `.zmx` format specification (SURF, CURV, DISZ, GLAS, CONI, DIAM, STOP), parser architecture, and canonical lens assemblies (Cooke Triplet, Double Gauss). |
| [**06. VectorCraft-Inspired Architecture**](research/reverse_engineering/06_vectorcraft_inspired_architecture.md) | Rust workspace blueprint (`crates/` & `apps/`), tokenized design system, egui 0.36 dockable panels, zero-panic clippy policy, and WASM/Native target strategy. |
| [**07. Deep-Dive REA Evidence**](research/reverse_engineering/07_deep_dive_rea_evidence.md) | Microscopic AST and semantic evidence reconstructed across `three-gpu-pathtracer`, `ray-optics`, `optiland`, and `vectorcraft`. |
| [**08. Aberrations & Paraxial Math**](research/reverse_engineering/08_optical_aberrations_and_paraxial_math.md) | First-order $y-u$ matrix transfer optics, cardinal points, Seidel aberrations, transverse ray aberration fans, and geometric spot diagram with Airy disk metrics. |
| [**09. Exhaustive File Inventory**](research/reverse_engineering/09_complete_file_inventory_and_architectural_trace.md) | Microscopic file-by-file dependency trace, module mapping, and architectural blueprint across all reference applications. |
| [**Unified Glass Database**](research/glass_catalogs/optical_glasses.json) | 646 real-world optical glasses from Schott, Ohara, CDGM, and Hoya with verified Sellmeier coefficients and Abbe numbers. |

---

## Core System Functionalities

### 1. Single-Ray Inspector
* **UI Visual:** 3D interactive viewport highlighting selected ray paths with ray-point debug overlays.
* **Functionality:** Click on any screen pixel to visualize its complete 3D trajectory (reflection, refraction, spectral dispersion, and termination) with per-bounce physical values (Snell's Law, Fresnel, BSDF).

### 2. Spectral Dispersion Engine
* **UI Visual:** Spectral breakdown display showing white light splitting into wavelength bands across transparent media.
* **Functionality:** Implements hero-wavelength spectral sampling (380–700 nm) and glass dispersion models (Cauchy/Sellmeier) to simulate physical rainbows, prisms, and chromatic aberration.

### 3. Acceleration Structure (BVH) Diagnostics
* **UI Visual:** Wireframe 3D bounding boxes (AABB) superimposed on models and a color-coded viewport heatmap overlay.
* **Functionality:** Visualizes per-pixel traversal cost based on the number of ray-triangle and ray-box intersection tests in WGSL compute passes.

### 4. Convergence & Analytics Panel
* **UI Visual:** Live line graphs and real-time spectral intensity histograms.
* **Functionality:** Tracks render convergence rates (RMSE/variance) over time and plots exact wavelength contributions for any selected pixel or region.

### 5. Multi-Target Core Architecture
* **UI Visual:** Unified single-interface application running as a native desktop executable or in a web browser tab.
* **Functionality:** Shared pure-Rust core compiled to native GPU targets (Vulkan, Metal, DirectX 12 via `wgpu`) and WebAssembly (WebGPU for browsers), featuring an embedded CPU reference tracer for side-by-side validation.

---

## Workspace Structure Plan

```
crates/
  optics-core/        # Ray primitives, 3D math, Snell/Fresnel vector algebra
  optics-spectral/    # Sellmeier & Cauchy formulas, Hero-wavelength sampling, CIE 1931 XYZ
  optics-glass/       # Real-world optical glass catalogs (Schott, Ohara, Hoya, CDGM)
  optics-bvh/         # 2-level TLAS/BLAS acceleration & traversal cost evaluation
  optics-zemax/       # Zemax .zmx file parser & lens prescription generator
  optics-render/      # wgpu path tracing pipeline, compute shaders, camera models
  optics-ui/          # egui design tokens, dockable panels, interactive canvas
apps/
  optics-lab/         # Native desktop application (macOS, Windows, Linux)
  optics-cli/         # Headless benchmark and batch diagnostic runner
  optics-web/         # WebAssembly (WASM + WebGPU) browser deployment
```

---

## License

MIT License. See [LICENSE](LICENSE) for details.
