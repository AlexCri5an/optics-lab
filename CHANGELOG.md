# Changelog

All notable changes to **OpticsLab** will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added

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
