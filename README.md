# OpticsLab

An open-source, double-precision computational optics engine and visual laboratory for lens design, spectral dispersion, ray tracing diagnostics, and physical simulations.

OpticsLab is a modern, pure-Rust alternative to commercial optical engineering suites (*Zemax OpticStudio*, *CODE V*, *LightTools*) and research tracers (*Optiland*, *rayoptics*). Built with zero-panic reliability, double-precision (`f64`) physics, and high-performance spatial indexing, it delivers real-time sequential ray tracing, paraxial transfer analysis, comprehensive aberration diagnostics (spot, wavefront, Zernike, Seidel, MTF, PSF), 646 bundled glasses, thin-film Transfer-Matrix Method (TMM) coatings, Jones/Mueller polarimetry, DLS optimization, tolerancing/Monte Carlo yield, 3D non-sequential ray tracing, and multi-format CAD interoperability.

---

## 1. Quick Start

### 1.1 Run CLI Self-Test & Diagnostics
```bash
# Execute full 10-suite automated self-test across all 12 crates
cargo run -p optics-cli -- --self-test

# Run lens analysis on canonical Cooke Triplet F/4.5 preset
cargo run -p optics-cli -- analyze --preset cooke-triplet

# Export technical drawings across formats (SVG, DXF, STL, OBJ, CODE V, OSLO, ISO 10110)
cargo run -p optics-cli -- export --preset cooke-triplet --format svg --out cooke_triplet.svg
cargo run -p optics-cli -- export --preset cooke-triplet --format dxf --out cooke_triplet.dxf
cargo run -p optics-cli -- export --preset cooke-triplet --format stl --out cooke_triplet.stl
cargo run -p optics-cli -- export --preset cooke-triplet --format codev --out cooke_triplet.seq

# Search the 646-glass manufacturer database (Schott, Ohara, CDGM, Hoya)
cargo run -p optics-cli -- glass search "N-BK7"
```

### 1.2 Run Standalone Examples
OpticsLab includes 10 self-contained runnable examples demonstrating every core capability:
```bash
cargo run -p optics-engine --example 01_singlet_optimization
cargo run -p optics-engine --example 02_cooke_triplet_analysis
cargo run -p optics-engine --example 03_tolerance_monte_carlo
cargo run -p optics-engine --example 04_prism_dispersion_nonseq
cargo run -p optics-engine --example 05_light_pipe_tir
cargo run -p optics-engine --example 06_cad_and_format_export
cargo run -p optics-engine --example 07_engine_facade_and_undo
cargo run -p optics-engine --example 08_thin_film_polarization
cargo run -p optics-engine --example 09_glass_catalog_search
cargo run -p optics-engine --example 10_aspheric_surfaces
```

---

## 2. Repository Layout

```text
/
├── .github/workflows/ci.yml   # GitHub Actions CI matrix workflow
├── apps/
│   └── optics-cli/            # Headless CLI diagnostic tool & exporter
├── crates/                    # 12 modular computational backend crates
│   ├── optics-core/           # 3D vectors, rays, Snell/Fresnel, aspheres, aberrations, TMM, polarimetry
│   ├── optics-spectral/       # Sellmeier/Cauchy/Schott dispersion, thermal shifts, CIE 1931 colorimetry
│   ├── optics-glass/          # 646 manufacturer optical glasses database, AGF and YAML loaders
│   ├── optics-bvh/            # Surface Area Heuristic (SAH) BVH & GPU buffer packing
│   ├── optics-zemax/          # Lossless .zmx roundtrip, CODE V, OSLO, SVG, DXF, STL, OBJ, ISO 10110
│   ├── optics-render/         # Headless hardware WebGPU path tracer & WGSL shaders
│   ├── optics-optimize/       # Damped Least Squares (DLS), multi-start, simulated annealing, glass hammer
│   ├── optics-tolerance/      # Sensitivity ranking, paraxial compensators, Monte Carlo yield simulation
│   ├── optics-nonseq/         # 3D non-sequential ray tracing, Fresnel branching, 2D detectors, TIR guides
│   ├── optics-engine/         # High-level facade, transactional command queue, undo/redo, dirty cache
│   └── optics-ffi/            # C99 FFI ABI boundary and Python ctypes bindings
├── docs/                      # Architectural and technical documentation
│   ├── API_REFERENCE.md       # Public API reference for OpticalEngine & foreign bindings
│   ├── API_SCHEMA.json        # JSON Schema for engine commands and analysis requests
│   ├── API_STABILITY.md       # SemVer guarantees and API stability tiers
│   ├── ARCHITECTURE.md        # 12-crate workspace architecture and pipeline diagrams
│   ├── CONVENTIONS.md         # Mathematical signs, optical units, and coordinate standards
│   ├── ROADMAP.md             # Technical roadmap and planned feature progression
│   ├── VALIDATION.md          # Verification guide for test execution and benchmark reproduction
│   ├── archive/               # Historical research traces and sign-off records
│   └── audit/                 # Consolidated STATUS.md, MATHEMATICAL_AUDIT.md, FUNCTION_INVENTORY.md
├── examples/                  # Symlink to crates/optics-engine/examples/ (10 runnable scripts)
├── include/
│   └── optics_lab.h           # Canonical C99 header for C ABI integration
└── tools/
    └── golden/                # Independent golden verification datasets and Python test harness
```

---

## 3. Verified Computational Features

- **Double-Precision Standard**: Every ray coordinate, direction cosine, surface normal, and optical path length is computed in `f64`.
- **Zero-Panic Guarantee**: Every workspace crate strictly enforces `#![deny(clippy::unwrap_used)]` and `#![deny(clippy::expect_used)]`. Errors propagate via typed `Result<T, OpticsError>`.
- **Golden Reference Validation**: Cross-validated against textbook benchmarks (Smith *Modern Optical Engineering*, Arthur Cox *A System of Optical Design*, and MIL-HDBK-141) with agreement within $0.002\% - 0.02\%$.
- **Physical Invariants**: Property-based invariant tests verify Snell cross-product conservation ($< 10^{-12}$), normal unit length ($1.0 \pm 10^{-12}$), Fresnel energy conservation ($R+T=1.000000000000$), ray reversibility ($< 10^{-12}$), and surface-by-surface Lagrange invariant conservation ($< 10^{-10}$).
- **Adversarial Fuzzing**: Stress-tested against null bytes, extreme floats (`NaN`, `Infinity`, subnormals), progressive file truncation, and random bitflip mutations with zero panics.

---

## 4. Comparison with Existing Tools

| Capability | OpticsLab | Zemax OpticStudio | CODE V | Optiland (Python) | rayoptics (Python) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Open Source / License** | **Yes (MIT/Apache-2.0)** | No (Proprietary) | No (Proprietary) | Yes (MIT) | Yes (BSD) |
| **Language & Safety** | **Pure Rust (Zero-Panic)**| C++ / C# | Fortran / C++ | Python | Python |
| **Precision Standard** | **Strict `f64`** | `f64` | `f64` | `f64` (NumPy) | `f64` (NumPy) |
| **Execution Speed** | **Native (~0.05ms solve)**| Native | Native | Interpreted (~50x slower)| Interpreted |
| **Sequential Ray Tracing** | **Full (Conic, Asphere, Forbes, Zernike)** | Full | Full | Partial | Partial |
| **Wavefront & Zernike OPD**| **Full (Noll 1–36, Gram-Schmidt)** | Full | Full | Partial | Partial |
| **Diffraction MTF & FFT PSF**| **Full (Radix-2 2D FFT)** | Full | Full | Partial | No |
| **Thin Films & Polarization**| **Full (TMM characteristic matrix, Jones/Mueller)**| Full | Full | No | No |
| **Optimization** | **DLS, Multi-Start, Annealing, Glass Hammer** | Full | Full | Partial (SciPy) | No |
| **Tolerancing & Monte Carlo**| **Sensitivity ranking, Compensators, MC percentiles**| Full | Full | No | No |
| **Non-Sequential Ray Tracing**| **3D Primitives, Fresnel branching, Detectors** | Full | Separate Mode | No | No |
| **Glass Catalogs Bundled** | **646 manufacturer glasses** | Vendor libraries | Vendor libraries | Limited | Limited |
| **Interoperability & CAD** | **ZMX, SEQ, LEN, SVG, DXF, STL, OBJ, ISO 10110** | ZMX | SEQ | Partial | Partial |
| **Frontend Integration** | **Unified Facade, Undo/Redo, C ABI, Python** | Desktop GUI | Desktop GUI | Notebooks | Notebooks |

---

## 5. Building & Testing

```bash
# Compile entire workspace
cargo build --workspace

# Run complete test suite (218 tests)
cargo test --workspace

# Run Clippy static analysis with zero-panic lint enforcement
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Build documentation
cargo doc --workspace --no-deps --open
```

---

## 6. License

Dual-licensed under either:
- **MIT License** ([LICENSE-MIT](LICENSE-MIT))
- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
