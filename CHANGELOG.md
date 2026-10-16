# Changelog

All notable changes to **OpticsLab** will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.0] - 2026-10-16

Initial release of the pure computational optics backend with zero-panic safety, strict `f64` precision, and high-performance cross-platform modular architecture.

### Added

#### Pure Optical Physics Engine (`crates/optics-core`)
- **Vector Mechanics & Refraction Algebra (`snell.rs`):**
  - Analytical Snell's Law in 3D vector form ($\mathbf{t} = \eta \mathbf{i} + (\eta c - c_t) \mathbf{n}$) with optical reversibility.
  - Total Internal Reflection (TIR) detection ($\sin^2 \theta_t > 1.0$) with zero transmission.
  - Surface normal canonicalization ($\mathbf{i} \cdot \mathbf{n} \le 0$) detecting and inverting flipped normals automatically.
  - Pure specular reflection vector evaluation ($\mathbf{r} = \mathbf{i} - 2(\mathbf{i} \cdot \mathbf{n})\mathbf{n}$).
- **Fresnel Dielectric Boundary Equations (`fresnel.rs`):**
  - Exact unpolarized reflectance ($R = \frac{1}{2}(R_s + R_p)$) and transmittance ($T = 1 - R$).
  - Exact s-polarized and p-polarized amplitudes and power coefficients across all incidence angles.
  - Schlick's polynomial approximation ($R(\theta) = R_0 + (1 - R_0)(1 - \cos\theta)^5$) with verified Brewster angle dip.
- **Conic & Advanced Optical Surfaces (`conic.rs`, `surface/`):**
  - Quadratic conic sag evaluation $z(r) = \frac{c r^2}{1 + \sqrt{1 - (1 + k) c^2 r^2}}$.
  - Even polynomial aspheres ($r^4 \dots r^{20}$) with Newton-Raphson intersection solver.
  - Odd polynomial aspheres, toroidal and biconic cross-cylinder surfaces.
  - Zernike standard sag surfaces (Noll indices $1 \dots 36$) and general XY polynomials.
  - Forbes orthogonal $Q$-type aspheres ($Q_{\text{con}}, Q_{\text{bfs}}$) via 3-term recurrence.
  - Diffractive / binary phase surfaces with vector grating phase polynomials.
  - Gradient-Index (GRIN) media with 4th-order Runge-Kutta (RK4) ray stepping.
  - 3D coordinate breaks (Euler tilt & decenter transformations).
  - Surface apertures: circular, rectangular, elliptical, annular, polygonal.
- **Paraxial Solvers & Field Modeling (`prescription.rs`, `aiming.rs`, `multiconfig.rs`):**
  - First-order $y-u$ matrix transfer ray tracing across spherical and planar interfaces.
  - Cardinal points: Effective Focal Length (EFL), Back Focal Length (BFL), Front Focal Length (FFL), Entrance/Exit Pupils.
  - Optical Lagrange Invariant conservation verification ($H = n(y u' - y' u) = \text{const}$).
  - 2D Newton-Raphson iterative pupil aiming at arbitrary stop surfaces.
  - Multi-configuration zoom systems with parameter overrides and pickups.
- **Aberrations & Wavefront Diagnostics (`analysis/`):**
  - Hexapolar, grid, and spiral entrance pupil sampling.
  - Geometric spot diagram evaluation: centroid, RMS spot radius, 100% geometric enclosure, Airy disc comparison.
  - Transverse ray aberration fans (Tangential / Sagittal planes).
  - Chromatic focal shift fan across visible spectrum.
  - Optical Path Difference (OPD) wavefront map with Hopkins reference sphere subtraction.
  - Gram-Schmidt orthogonal Zernike polynomial decomposition ($j=1\dots 36$).
  - Seidel 3rd-order aberration coefficients ($S_I, S_{II}, S_{III}, S_{IV}, S_V, C_L, C_T$) and Petzval sum.
  - 2D FFT diffraction Point Spread Function (PSF) and radial Encircled Energy (EE).
  - Polychromatic diffraction Modulation Transfer Function (MTF) up to cutoff frequency.
  - Astigmatic field curves, through-focus MTF, and double-bounce ghost reflection paths.
- **Polarization & Thin-Film Coatings (`polarization.rs`, `thin_film.rs`):**
  - 3D polarization ray tracing matrix ($P$-matrix) preserving electric field orientation.
  - Jones vectors, Stokes parameters, and Mueller matrix transformations.
  - $4\times 4$ Transfer Matrix Method (TMM) multilayer thin-film solver with complex Fresnel coefficients.

#### Spectral Dispersion & Colorimetry (`crates/optics-spectral`)
- **Dispersion & Atmospheric Physics (`dispersion.rs`, `thermal.rs`):**
  - Schott Sellmeier 1, Sellmeier 2, Cauchy, Extended Cauchy, Schott, Herzberger, and Conrady formulas.
  - Edlén and Birch & Downs high-precision atmospheric air refractive index model.
  - Schott temperature-dependent dispersion shifts ($dn/dT$) and internal linear transmittance $\alpha(\lambda)$.
- **CIE 1931 Colorimetry & Sampling (`color.rs`, `hero.rs`):**
  - CIE 1931 $2^\circ$ standard colorimetric observer matching functions ($\bar{x}, \bar{y}, \bar{z}$).
  - CIE Standard Illuminant D65 spectral power distribution table.
  - IEC 61966-2-1 continuous sRGB conversion.
  - Unbiased Monte Carlo hero-wavelength spectral bundle sampling.

#### Optical Glass Database (`crates/optics-glass`)
- **646 Manufacturer Glasses (`catalog.rs`, `material.rs`):**
  - Bundled database of 646 materials from CDGM, Schott, Ohara, Hikari, Sumita, and Hoya.
  - Abbe diagram 2D querying ($n_d, V_d$) and lead-free alias normalization.
  - Synthetic model glass solver with linear glass map interpolation.
  - Zemax `.agf` catalog parser and RefractiveIndex.INFO YAML loader.

#### 2-Level BVH Acceleration Structure (`crates/optics-bvh`)
- **Spatial Indexing & GPU Packing (`builder.rs`, `scene.rs`, `gpu.rs`, `tessellate.rs`):**
  - Surface Area Heuristic (SAH) builder with 16 binned split planes per axis.
  - 2-level TLAS/BLAS instance hierarchy with dynamic object transforms.
  - Rotational conic lens and outer rim watertight triangular mesh generators.
  - std430-aligned 32-byte GPU buffer packing for compute shaders.

#### Interoperability & File Formats (`crates/optics-zemax`)
- **Optical Format Bridges (`parser.rs`, `serializer.rs`, `codev.rs`, `oslo.rs`):**
  - Lossless Zemax `.zmx` token parser and bidirectional serializer preserving all metadata operands.
  - Synopsys CODE V `.seq` reader and writer.
  - Lambda Research OSLO `.len` reader and writer.
  - Versioned JSON project workspace serialization (`schema_version = "1.0.0"`).
- **Technical Drawing & 3D CAD Exporters (`cad.rs`, `iso10110.rs`):**
  - 2D SVG vector technical diagrams with ray trace paths.
  - AutoCAD Release 12 2D and 3D DXF technical drawings.
  - 3D STL binary and ASCII watertight mesh exporter.
  - Wavefront OBJ with companion MTL material definition library.
  - ISO 10110-10 manufacturing drawing specification generator.

#### GPU Path Tracing Pipeline (`crates/optics-render`)
- **Headless WebGPU Renderer (`pipeline.rs`, `shaders/`):**
  - Hardware-accelerated compute pass in WGSL (`path_tracer.wgsl`).
  - Dynamic Sellmeier dispersion evaluated in compute shaders via multi-material storage buffers.
  - Progressive accumulation buffer and ACES tonemapping pass (`tonemapping.wgsl`).
  - Thin-lens physical camera model with depth of field and pupil sampling.
  - Online Welford variance tracking and RMSE convergence metrics.

#### Optical Optimization Engine (`crates/optics-optimize`)
- **DLS & Global Solvers (`dls.rs`, `global.rs`, `hammer.rs`, `merit.rs`):**
  - Levenberg-Marquardt Damped Least Squares (DLS) algorithm with central finite-difference Jacobian.
  - Merit function operand architecture (EFL, BFL, F/#, RMS spot, RMS wavefront, Seidel, MTF, thicknesses).
  - Global multi-start basin hopping and simulated annealing.
  - Discrete glass catalog substitution ("Hammer" optimizer).

#### Manufacturing Tolerancing Engine (`crates/optics-tolerance`)
- **Sensitivity & Monte Carlo Analysis (`sensitivity.rs`, `monte_carlo.rs`, `inverse.rs`):**
  - Worst-offender sensitivity ranking across radii, thicknesses, conics, tilts, decenters, and index.
  - Uniform, Truncated Normal, and Worst-Case statistical error distributions.
  - Analytical paraxial refocus compensator matching closed-form focus shift.
  - Root-Sum-Square (RSS) error budgeting and deterministic Monte Carlo yield simulation (50th, 90th, 99th percentiles).

#### 3D Non-Sequential Ray Tracing (`crates/optics-nonseq`)
- **Radiometric Ray Tracer (`trace.rs`, `geometry.rs`, `detector.rs`):**
  - Arbitrary 3D non-sequential geometry (Plane, Disk, Sphere, Cylinder, Box, Mesh).
  - Recursive tree tracing with vector Snell/Fresnel energy branching ($1.000000$ energy conservation factor).
  - 2D pixelated radiometric (irradiance $\text{W/m}^2$) and photopic (illuminance $\text{lux}$) detectors.
  - Total Internal Reflection (TIR) light guide simulation with zero numerical energy leakage.

#### Unified Engine Facade (`crates/optics-engine`)
- **High-Level Orchestration (`engine.rs`, `commands.rs`, `events.rs`, `jobs.rs`):**
  - Reversible command pattern (`EngineCommand`) with bounded undo/redo history (`UndoStack`).
  - Revision-keyed cached analysis evaluation with dirty tracking.
  - Diagnostic validation layer (`DiagnosticLevel`) checking geometry and physical parameters.
  - Reactive event broadcasting (`EngineEvent`) for GUI frontends.
  - Background worker thread execution for compute-intensive jobs (`JobHandle`).

#### C FFI ABI Boundary (`crates/optics-ffi`)
- **Foreign Language Integration (`lib.rs`, `include/optics_lab.h`):**
  - Opaque pointer handle (`OpticsEngineHandle`) with thread-local error messaging.
  - Complete C99 header declaring transactional commands, queries, and CAD exports.
  - Zero-dependency Python ctypes test harness and client integration.

#### CLI & Runnable Examples (`apps/optics-cli`, `examples/`)
- Thin CLI tool with `self-test`, `analyze`, `export`, and `glass` commands.
- 10 standalone, runnable examples demonstrating every subsystem with zero panics.
