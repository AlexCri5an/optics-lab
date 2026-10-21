# Contributing to OpticsLab

Thank you for your interest in contributing to **OpticsLab**!
OpticsLab is a clean-room, open-source, pure-Rust optical engineering and raytracing CAD workbench.
We welcome contributions from optical engineers, physicists, mathematicians, and software developers.

---

## 1. Non-Negotiables & Guiding Principles

Every contribution must adhere strictly to these core tenets:

### A. Clean-Room Development (Absolute Rule)
* **Never use decompiled, reverse-engineered, or proprietary code**: Commercial optical software terms of use forbid reverse engineering. Never inspect binaries, DLLs, proprietary file formats, or disassemblies of commercial CAD or raytracing software.
* **Public scientific literature only**: All optical algorithms, dispersion formulas, paraxial equations, and aberration expansions must derive strictly from published academic textbooks (e.g. Born & Wolf, Kingslake, Smith), ISO standards (e.g. ISO 10110), or mathematical first principles.
* **No GPL/AGPL contamination**: Never copy or import code licensed under GPL, AGPL, or other viral copyleft licenses. All external dependencies must be strictly permissive.

### B. Never Crash: No-Panic Policy
* Production crates must **never panic on user input, numerical singularity, or file import**.
* All fallible operations must return `Result<T, OpticsError>` and propagate errors with `?`.
* In non-test production code, workspace Clippy lints forbid bare `.unwrap()`, `.expect()`, `panic!`, `todo!`, or `unimplemented!`.
* Check numerical limits: non-zero radii of curvature, positive refractive index, finite vectors, and valid ray direction normalization.

### C. Architectural Layering
OpticsLab maintains a strict unidirectional layering architecture:
1. **Mathematical Primitives & Physical Physics**:
   * `crates/optics-core`: Vectors, paraxial tracing, Seidel surface sums, ray definitions. Zero heavy dependencies.
   * `crates/optics-spectral`: Fraunhofer emission lines, spectral distributions, CIE colorimetry.
   * `crates/optics-glass`: Embedded 646-material catalog, Sellmeier & Cauchy dispersion models.
2. **Computational Engines**:
   * `crates/optics-engine`: High-level optical system prescription, surface sequential raytracing, spot diagrams, transverse ray fans.
   * `crates/optics-optimize`: Damped Least Squares (DLS / Levenberg-Marquardt) optimization and operands.
   * `crates/optics-nonseq`: Non-sequential Monte Carlo raytracer with BVH acceleration, thin-film coatings, and ghost reflections.
   * `crates/optics-tolerance`: RSS and Monte Carlo manufacturing tolerance analyzer.
3. **CAD & Technical Drawings**:
   * `crates/optics-formats`: AutoCAD Release 12 DXF, SVG diagrams, Wavefront OBJ, STL 3D printing, and ISO 10110 reports.
4. **User Interface & Applications**:
   * `crates/optics-ui`: Modern academic UI components (`egui`), 2D optical layout viewport, and typed Lucide vector icons.
   * `apps/optics-desktop`: Native desktop application entry point (`OpticsLab`).
   * `apps/optics-cli`: Command-line interface for batch raytracing and headless automation.

Nothing below the UI layer may depend on `egui`, `eframe`, `wgpu`, or windowing libraries.

### D. Asset Policy
* Every non-code asset (image, icon, scientific catalog, benchmark fixture) must be recorded in [`ASSETS.md`](ASSETS.md) with its author, source, and licence.
* Third-party licences must be summarized in [`NOTICE`](NOTICE).
* No unlicensed or proprietary binary files may ever be committed.

---

## 2. Licensing (Inbound = Outbound)

OpticsLab is dual-licensed under:
* **MIT License** ([LICENSE-MIT](LICENSE-MIT))
* **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE))

By opening a Pull Request or submitting code to this project, you explicitly agree that your contribution is licensed under this dual MIT/Apache-2.0 license, without additional restrictions or proprietary encumbrances.

---

## 3. Development Setup & Quality Gates

### Prerequisites
* **Rust**: 1.87 or newer (stable toolchain, Edition 2024).
* **Components**: `rustfmt` and `clippy`.

```bash
rustup update stable
rustup component add clippy rustfmt
```

### Running Quality Gates Locally
Before pushing your branch or opening a PR, all four gates must pass cleanly:

```bash
# 1. Format check
cargo fmt --all -- --check

# 2. Strict Clippy check (must pass with zero warnings)
cargo clippy --workspace --all-targets -- -D warnings

# 3. Complete workspace test suite
cargo test --workspace

# 4. Native desktop build validation
cargo build -p optics-desktop
```

---

## 4. Submitting a Pull Request

1. **Fork and branch**: Create a descriptive feature branch from `main` (`git checkout -b feat/aspheric-q-type`).
2. **Commit conventions**: Use concise, imperative commit titles:
   * `feat(spectral): add Cauchy dispersion model`
   * `fix(engine): resolve paraxial marginal ray solve at stop`
   * `perf(bvh): vectorize bounding box intersection`
   * `docs(readme): update quickstart examples`
   * `test(tolerance): add Monte Carlo percentile distribution tests`
3. **Include tests**: Add unit tests in the same crate (`#[cfg(test)]`) or integration tests in `tests/` covering both nominal cases and edge cases (singularities, grazing incidence, total internal reflection).
4. **Open a PR**: Fill out the Pull Request description, linking any relevant issues and describing the optical theory or mathematical derivation used.

---

## 5. Reporting Issues

* Use the GitHub Issue Tracker.
* When reporting numerical discrepancies, please cite the academic source (author, title, equation number, page) or provide an analytical derivation.
* For bug reports, include your OS version, GPU hardware, and minimal optical prescription.
