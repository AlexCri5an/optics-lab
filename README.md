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

## 2. Workspace Crate Architecture

The codebase consists of 12 focused, modular crates:

| Crate | Purpose | Scope / Capabilities |
| :--- | :--- | :--- |
| [`optics-core`](crates/optics-core) | Core Math & Physics | 3D vectors (`Vec3`), parametric rays (`Ray`), Snell/Fresnel vector algebra, conic/aspheric/Forbes/Zernike/GRIN surfaces, paraxial ABCD transfer, ray aiming, spot diagram, wavefront OPD, Zernike fit, Seidel sums, FFT PSF, diffraction MTF, TMM coatings, Jones/Mueller polarimetry. |
| [`optics-spectral`](crates/optics-spectral) | Spectral Physics | Sellmeier 1–5, Schott, Herzberger, Conrady, and extended dispersion; Edlén / Birch & Downs atmospheric air model; Schott thermal dispersion ($dn/dT$); internal linear transmittance; CIE 1931 XYZ $\to$ sRGB; hero wavelength sampling. |
| [`optics-glass`](crates/optics-glass) | Optical Materials | 646 bundled manufacturer glasses (Schott, Ohara, CDGM, Hoya); Abbe diagram querying ($n_d, V_d$); synthetic model glass; Zemax `.agf` and refractiveindex.info YAML parsers. |
| [`optics-bvh`](crates/optics-bvh) | Spatial Acceleration | Surface Area Heuristic (SAH) builder; Möller-Trumbore ray-triangle intersection; 2-level TLAS/BLAS instance traversal; GPU buffer packing for compute shaders. |
| [`optics-zemax`](crates/optics-zemax) | Interoperability & CAD | Lossless Zemax `.zmx` parser/serializer; canonical presets (Cooke Triplet, Double Gauss, Singlet); CODE V `.seq` & OSLO `.len` bridges; SVG 2D layout; AutoCAD DXF; 3D STL binary/ASCII; Wavefront OBJ + MTL; ISO 10110 drawing sheets; versioned project workspace schema. |
| [`optics-render`](crates/optics-render) | GPU Path Tracing | Hardware WebGPU path tracer (`wgpu`); WGSL compute shaders; multi-material Sellmeier storage buffer indexing; thin-lens camera; Welford online variance tracking. |
| [`optics-optimize`](crates/optics-optimize) | System Optimization | Levenberg-Marquardt Damped Least Squares (DLS); finite-difference Jacobian; comprehensive merit function operands; multi-start global search; simulated annealing; discrete glass catalog substitution ("Hammer"). |
| [`optics-tolerance`](crates/optics-tolerance) | Manufacturing Tolerancing | Radius, thickness, conic, decenter, tilt, wedge, index/Abbe tolerances; Uniform, Truncated Normal, Worst-Case distributions; sensitivity ranking; paraxial refocus compensator; RSS error budget; deterministic Monte Carlo yield simulation. |
| [`optics-nonseq`](crates/optics-nonseq) | 3D Non-Sequential | Mixed primitive geometry (Plane, Disk, Sphere, Cylinder, Box, Mesh); vector Snell/Fresnel branching; Total Internal Reflection (TIR); Beer-Lambert absorption; 2D pixelated radiometric/photopic detectors; ray history logs. |
| [`optics-engine`](crates/optics-engine) | Unified Facade | Single integration target for UI frontends. Command pattern (`EngineCommand`), bounded undo/redo history (`UndoStack`), revision-keyed result caching with dirty invalidation, diagnostic validation layer (`DiagnosticLevel`), reactive event broadcasting (`EngineEvent`), background worker threads (`JobHandle`). |
| [`optics-ffi`](crates/optics-ffi) | Foreign Language C ABI | `extern "C"` ABI boundary, opaque handles (`OpticsEngineHandle`), thread-local error messaging, C header (`include/optics_lab.h`), zero-dependency Python `ctypes` bindings. |
| [`optics-cli`](apps/optics-cli) | Command-Line Interface | Thin CLI client over `optics-engine` providing command-line analysis, multi-format CAD exports, and automated 10-suite self-tests. |

---

## 3. Verified Computational Features

- **Double-Precision Standard**: Every ray coordinate, direction cosine, surface normal, and optical path length is computed in `f64`.
- **VectorCraft Zero-Panic Guarantee**: Every workspace crate strictly enforces `#![deny(clippy::unwrap_used)]` and `#![deny(clippy::expect_used)]`. Errors propagate via typed `Result<T, OpticsError>`.
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
cargo test --workspace && cargo test -p optics-bvh

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
