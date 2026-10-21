# Changelog

All notable changes to **OpticsLab** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-10-22

### Added
- **Sequential Optical Engine**:
  - Full spherical surface sequential raytracing with 3D coordinate transformation.
  - Paraxial optics solvers (effective focal length, back focal length, entrance/exit pupils, numerical aperture).
  - Third-order Seidel aberration coefficients calculation ($S_I$ Spherical, $S_{II}$ Coma, $S_{III}$ Astigmatism, $S_{IV}$ Field Curvature, $S_V$ Distortion).
  - Transverse ray aberration fan analysis and spot diagram computation.
- **Dispersion & Glass Catalog**:
  - Embedded catalog of 646 verified optical glasses (Schott, Ohara, CDGM, Hoya, Hikari, Sumita).
  - Multi-formula dispersion engine (Sellmeier 1 & 2, Laurent polynomial, Cauchy, and temperature/pressure scaling).
  - Standard Fraunhofer spectral emission lines (F, d, C, e, g, r, t).
- **Optimization & Tolerancing**:
  - Damped Least Squares (DLS / Levenberg-Marquardt) optical parameter optimizer.
  - Comprehensive merit function operands ($EFL$, spot radius, aberration targets, boundary thickness constraints).
  - Monte Carlo and Root-Sum-Square (RSS) tolerancing engine for manufacturing error margins.
- **Non-Sequential Monte Carlo Raytracer**:
  - High-performance Bounding Volume Hierarchy (BVH) ray traversal.
  - Fresnel reflection, thin-film anti-reflective coatings, transmission, and ghost reflections.
- **CAD & Technical Drawing Export**:
  - AutoCAD Release 12 DXF 2D lens cross-section layout export.
  - Scalable Vector Graphics (SVG) publication-ready ray diagram generator.
  - 3D Stereolithography (STL) mesh generation for rapid prototyping and 3D printing.
  - 3D Wavefront OBJ multi-material export.
  - ISO 10110 compliant optical manufacturing drawing report generator.
- **User Interface**:
  - Desktop workbench built with `egui` and `wgpu`.
  - Responsive academic TopBar with typed Lucide vector icons.
  - Interactive 2D optical layout viewport with pan, zoom, ray-path toggles, and pupil displays.
  - Optical prescription editor table with real-time solve updates.
- **C-Compatible FFI & CLI Automation**:
  - `optics-ffi` native C library interface (`extern "C"`) and C header (`include/optics_lab.h`).
  - `optics-cli` automation tool for headless raytracing, analysis, and SVG export.
- **Cross-Platform Packaging**:
  - macOS drag-and-drop DMG installer with Retina `.icns` and squircle icon clipping.
  - Linux standalone tarball and desktop integration.
  - Windows portable ZIP and WiX MSI installer configuration.
