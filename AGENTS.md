# OpticsLab — Instructions for Agents

OpticsLab is a clean-room, open-source, pure-Rust optical engineering and raytracing CAD workbench.
It provides rigorous physical simulation, paraxial optics, Seidel third-order aberration analysis, damped least squares (DLS) optimization, non-sequential Monte Carlo raytracing with BVH acceleration, ISO 10110 manufacturing export, and an interactive desktop UI built on `egui` and `wgpu`.

---

## Start Every Session Here

1. **Read `AGENTS.md` and `CONTRIBUTING.md`**: Understand the architecture layering, non-negotiable clean-room boundaries, and robustness requirements.
2. **Follow the autonomous engineering cycle**:
   `orient` → `plan` → `implement + test` → `verify quality gates` → `commit`.
   Do not stop to ask questions on purely mechanical tasks or standard refactorings; ask only when requirements are genuinely ambiguous.
3. **Verify the 4 Quality Gates before declaring any task complete**:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   cargo build -p optics-desktop
   ```

---

## Non-Negotiables

### 1. Clean-Room: Never Use Proprietary Software or Decompiled Code
* Commercial optical software (Zemax OpticStudio, Code V, OSLO, SYNOPSYS) terms of use forbid reverse engineering and competitive analysis.
* **Absolute rules**:
  - Never launch, script, automate, or screenshot commercial optical CAD software to compare output.
  - Never inspect, disassemble, decompiled, or reverse-engineer commercial optical binaries, DLLs, or proprietary encrypted glass files.
  - Never copy proprietary sample files, macro languages (ZPL), or internal schemas. Write test inputs from scratch based on public optical prescriptions (Cooke triplet, Double Gauss, Petzval, Tessar).
  - All mathematical algorithms, paraxial optics equations, Seidel surface expansions, and dispersion formulas derive strictly from public academic physics literature (Born & Wolf *Principles of Optics*, Kingslake *Lens Design Fundamentals*, Warren J. Smith *Modern Optical Engineering*) or international public standards (ISO 10110).
  - Never import or copy GPL/AGPL-licensed libraries.

### 2. Assets: No Proprietary Iconography or Unattributed Binaries
* Never add, trace, copy, or embed any icon, artwork, or asset from commercial optical software or sources without an open, permissive licence.
* Every asset file must be:
  - Original work created for OpticsLab by contributors (licensed under MIT OR Apache-2.0);
  - Public domain / CC0; or
  - Permissive open-source (MIT, Apache-2.0, ISC, SIL OFL-1.1).
* **Every asset file must have a row in [`ASSETS.md`](ASSETS.md)** (path, author, source, licence, notes), and third-party terms must be summarized in [`NOTICE`](NOTICE).
* Prefer procedurally generated vector curves in code for UI controls (`crates/optics-ui/src/icons.rs`) rather than external image binaries.

### 3. Never Crash: No Panics in Shipped Code
* A crash loses the optical designer's unsaved prescription and simulation state.
* Prescriptions, optical files, CLI parameters, and UI edits are all untrusted inputs.
* **Workspace Clippy lints enforce**:
  - `unwrap_used = "deny"`
  - `expect_used = "deny"`
  - Outside of tests (`#[test]`, `#[cfg(test)]`, and `tests/`), never call `.unwrap()`, `.expect()`, `panic!`, `todo!`, or `unimplemented!`.
  - Fallible operations return `Result<T, OpticsError>` and propagate errors with `?`.
  - Use `?`, `.ok_or(...)?`, `if let`, `let ... else { return ... }`, `unwrap_or_default()`, or direct struct initialization.
* Prevent implicit panics: validate array indexing with `.get()`, guard integer divisions against zero divisors, check that floating-point numbers are finite (`x.is_finite()`), and clamp radiometric flux and wavelengths to physically valid domains.

### 4. Architectural Layering
Layering is strictly unidirectional. Lower layers must never depend on higher layers:
* **Layer 1 — Core Physics & Primitives (`crates/optics-core`, `crates/optics-spectral`, `crates/optics-glass`)**:
  - 3D vectors, rays, paraxial transfer matrices, Seidel sums, Fraunhofer spectral lines, Sellmeier dispersion, and 646 embedded optical materials. Zero heavy dependencies.
* **Layer 2 — Spatial Acceleration & Rendering Engine (`crates/optics-bvh`, `crates/optics-render`)**:
  - Bounding Volume Hierarchy (BVH) construction, ray-triangle intersection, and headless GPU compute shaders.
* **Layer 3 — Computational Solvers (`crates/optics-engine`, `crates/optics-optimize`, `crates/optics-nonseq`, `crates/optics-tolerance`)**:
  - Optical prescription management, surface sequential raytracer, spot diagrams, transverse ray fans, DLS optimization, Monte Carlo non-sequential tracer with ghost analysis, and RSS tolerancing.
* **Layer 4 — CAD & Serialization (`crates/optics-formats`, `crates/optics-ffi`)**:
  - AutoCAD Release 12 DXF, SVG publication diagrams, Wavefront OBJ, STL 3D printing, ISO 10110 reports, and C-compatible FFI.
* **Layer 5 — User Interface (`crates/optics-ui`)**:
  - Academic dark theme, responsive docking, 2D cross-section viewport, ray renderers, and Lucide vector icons.
* **Layer 6 — Application Entry Points (`apps/optics-desktop`, `apps/optics-cli`)**:
  - Desktop binary executable (`OpticsLab`) and CLI automation runner.

**Rule**: Nothing below Layer 5 may depend on `egui`, `eframe`, `wgpu`, or windowing libraries.

### 5. Everything is a Facade Command
* User-visible optical modifications and prescription edits operate through the `EngineCommand` facade in `crates/optics-engine/src/commands.rs`.
* Enables clean undo/redo history, batch execution, and headless automation.

### 6. Code & Licensing
* Code is dual-licensed under **MIT OR Apache-2.0** (`LICENSE-MIT`, `LICENSE-APACHE`).
* Inbound contributions follow **Inbound = Outbound**: all submitted code is contributed under this dual license.
* Rust only: pure, safe, modern Rust (edition 2024, MSRV 1.87+).

---

## Running and Testing the Application

### 1. Launch the Desktop Workbench
```bash
cargo run --release -p optics-desktop
```
* **macOS**: The app runs natively with squircle window and dock icon branding, and the menu bar displays `OpticsLab`.
* **Packaging**: Use `./packaging/macos/package.sh` to generate a drag-and-drop macOS DMG installer with Retina `.icns`.

### 2. Headless CLI Automation
```bash
# Display help and available commands
cargo run --release -p optics-cli -- --help

# Run raytrace analysis on a built-in preset and export technical drawings
cargo run --release -p optics-cli -- analyze --preset cooke-triplet --export layout.svg
```

### 3. Testing Quality Gates
```bash
# Run unit tests and golden benchmarks across the entire workspace
cargo test --workspace

# Run formatting and strict clippy lints
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## Contributor Attribution

* All contributors and authors are credited in the repository history, release notes, and documentation.
* Non-code assets, fonts, icons, and scientific material property datasets are cataloged in [`ASSETS.md`](ASSETS.md) and summarized in [`NOTICE`](NOTICE).
