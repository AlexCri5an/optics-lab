# Changelog

All notable changes to **OpticsLab** will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.0] - 2026-10-16

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
- **Conic & Aspheric Surface Mathematics (`conic.rs`):**
  - Quadratic conic sag evaluation $z(r) = \frac{c r^2}{1 + \sqrt{1 - (1 + k) c^2 r^2}}$.
  - Continuous evaluation near planar zero curvature ($c \to 0$).
  - Analytic surface normal calculation $\mathbf{n} = \frac{(-c x, -c y, 1)}{\sqrt{1 - k c^2 r^2}}$.
  - Quadratic ray-conic surface intersection solver with aperture radius clipping.
- **Non-Sequential Path Tracing (`path.rs`, `geometry.rs`):**
  - Möller-Trumbore ray-triangle intersection on 3D scenes.
  - Branching policy (`FollowTransmission`, `FollowReflection`, `Terminated`).
  - Spectral ray fan evaluation across arbitrary wavelength bands.
- **Sequential Optical Prescriptions & Paraxial Transfer (`prescription.rs`):**
  - First-order $y-u$ matrix transfer ray tracing across spherical and planar interfaces.
  - Paraxial marginal and chief ray tracing.
  - Exact cardinal points computation: Effective Focal Length (EFL), Back Focal Length (BFL), Front Focal Length (FFL), Entrance/Exit Pupils.
  - Optical Lagrange Invariant conservation verification ($H = n(y u' - y' u) = \text{const}$).
  - Real 3D sequential ray tracing through multi-element optical assemblies with aperture stops and vignetting checks.
- **Aberration & Diagnostic Analysis (`analysis.rs`):**
  - Transverse ray aberration fans (Tangential / Sagittal planes).
  - Hexapolar, square grid, and Archimedean spiral entrance pupil ray pattern sampling.
  - Geometric spot diagram evaluation: centroid $(\bar{x}, \bar{y})$, RMS spot radius, arithmetic mean radius, $100\%$ geometric enclosure, and diffraction-limited Airy disk radius ($r_{\text{Airy}} = 1.22 \lambda F/\#$).

#### Spectral Dispersion & Colorimetry (`crates/optics-spectral`)
- **Physical Dispersion Models (`sellmeier.rs`, `cauchy.rs`):**
  - Schott Sellmeier 1 formula ($n^2(\lambda) = 1 + \sum_{i=1}^3 \frac{B_i \lambda^2}{\lambda^2 - C_i}$).
  - Schott Sellmeier 2 formula.
  - Cauchy polynomial dispersion model ($n(\lambda) = A + \frac{B}{\lambda^2} + \frac{C}{\lambda^4}$).
  - Abbe dispersion number $V_d = \frac{n_d - 1}{n_F - n_C}$ computed at Fraunhofer $d$ (587.56 nm), $F$ (486.13 nm), and $C$ (656.27 nm) lines.
- **CIE 1931 Standard Colorimetry & sRGB (`color.rs`):**
  - CIE 1931 $2^\circ$ standard colorimetric observer matching functions $\bar{x}(\lambda), \bar{y}(\lambda), \bar{z}(\lambda)$ tabulated from 380 nm to 780 nm.
  - CIE Standard Illuminant D65 spectral power distribution table (6504 K phase of daylight).
  - IEC 61966-2-1 piecewise continuous sRGB transfer function with linear toe and $2.4$ gamma exponent.
- **Hero-Wavelength Spectral Sampling (`hero.rs`):**
  - Unbiased Monte Carlo spectral bundle sampling (hero wavelength $\lambda_0$ + 3 equidistant companions $\lambda_j = (\lambda_0 + j \frac{\Delta\lambda}{4}) \pmod{\Delta\lambda}$).
  - Wavelength-to-RGB radiometric reconstruction preserving energy conservation.

#### Optical Glass Database (`crates/optics-glass`)
- **646-Glass Embedded Dataset (`catalog.rs`, `material.rs`):**
  - Full catalog embedded in binary via `include_bytes!` from Schott, Ohara, CDGM, and Hoya data sheets.
  - Case-insensitive name lookup with lead-free `N-` prefix alias resolution (e.g. `SK16` $\to$ `N-SK16`, `N-F2` $\to$ `F2`, `Fused Silica` $\to$ `LITHOSIL-Q`).
  - Abbe diagram 2D interval filtering (`search_by_nd_vd`) by index $n_d$ and Abbe number $V_d$.
  - Thread-safe `MaterialResolver` trait implementation for dynamic prescription binding.

#### 2-Level BVH Acceleration Structure (`crates/optics-bvh`)
- **SAH Binned BVH Builder (`builder.rs`, `aabb.rs`):**
  - Surface Area Heuristic (SAH) builder with 16 split bins per axis.
  - Fast ray-AABB slab intersection test with early exit.
  - 2-level TLAS/BLAS hierarchy supporting dynamic object instancing and transforms.
- **GPU Linear Buffer Packing (`gpu.rs`, `cost.rs`):**
  - Packed 32-byte bytemuck-aligned node structure for GPU compute upload.
  - Traversal cost telemetry recording ray-box and ray-triangle intersection counts.

#### Zemax .zmx Optical Specification Suite (`crates/optics-zemax`)
- **Parser & Serializer (`parser.rs`, `serializer.rs`):**
  - Complete `.zmx` token parser supporting `SURF`, `CURV`, `DISZ`, `GLAS`, `CONI`, `DIAM`, `STOP`.
  - Automatic UTF-8 and UTF-16LE with BOM encoding detection.
  - Canonical benchmark optical prescriptions:
    - Cooke Triplet F/4.5 (50 mm EFL, 3 elements, 6 surfaces)
    - Double Gauss F/2.0 (100 mm EFL, 6 elements, 10 surfaces)
    - Plano-Convex Singlet (100 mm EFL, 1 element, 2 surfaces)
- **Lossless Prescriptions:**
  - Full bidirectional serialization (`OpticalPrescription` $\leftrightarrow$ `.zmx`).

#### GPU Path Tracing Compute Pipeline (`crates/optics-render`)
- **wgpu Compute Pipeline (`pipeline.rs`, `shaders/`):**
  - Hardware-accelerated path tracing compute pass in WGSL (`path_tracer.wgsl`).
  - Dynamic Sellmeier glass dispersion evaluated directly inside GPU compute shaders.
  - Progressive accumulation buffer with tone-mapping compute pass (`tonemap.wgsl`).
  - Thin-lens physical camera model with depth of field, pupil radius, and focus distance.
  - Online Welford variance tracking and RMSE convergence calculation against reference buffers (`convergence.rs`).

#### Headless Diagnostic CLI (`apps/optics-cli`)
- Headless subcommands:
  - `prism`: traces D65 spectral fan through physical prisms, reporting angular deviation and Fresnel loss per wavelength.
  - `analyze`: runs paraxial ABCD, ray fans, and spot diagrams on lens presets or custom `.zmx` files.
  - `glass`: searches, filters, and inspects optical glasses from the 646-material catalog.
  - `benchmark`: runs re-trace latency and micro-benchmarks.
  - `self-test`: comprehensive hardware, GPU adapter, and numerical verification suite.
