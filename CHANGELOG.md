# Changelog

All notable changes to **OpticsLab** will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

> Work in progress — tracked in [`TODO.md`](TODO.md).

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
