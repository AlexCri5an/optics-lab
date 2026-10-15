# Reverse Engineering Document 06: VectorCraft-Inspired Architecture for OpticsLab

## 1. Executive Summary & Design Philosophy
**VectorCraft** (Brandon Thomas) revolutionized open-source creative software by proving that a clean-room, pure-Rust desktop application can mirror commercial giants like Adobe Illustrator in performance, UI polish, and cross-platform flexibility.

**OpticsLab** adopts the same gold-standard engineering principles to replicate and surpass commercial optical suites (Zemax OpticStudio, LightTools, CODE V) and research path tracers (Mitsuba 3, pbrt-v4).

---

## 2. Core Architectural Pillars

### 2.1 Workspace Crate Separation
Like VectorCraft (`crates/engine`, `crates/geom`, `crates/color`, `crates/ui-egui`), OpticsLab is divided into focused, single-responsibility Rust crates:

```
optics-lab/
├── Cargo.toml                  # Workspace root (resolver = "3", edition = "2024")
├── crates/
│   ├── optics-core/            # Ray representation, 3D math, Snell/Fresnel, Stokes polarization
│   ├── optics-spectral/        # Sellmeier/Cauchy equations, 380-700nm hero-sampling, CIE 1931 XYZ
│   ├── optics-glass/           # Glass database (Schott, Ohara, Hoya, CDGM, Fused Silica)
│   ├── optics-bvh/             # 2-level TLAS/BLAS, ray-box slab intersection, cost counting
│   ├── optics-zemax/           # .zmx parser, lens prescriptions (Cooke Triplet, Double Gauss)
│   ├── optics-render/          # wgpu path tracing pipeline, compute shaders, camera models
│   └── optics-ui/              # egui design tokens, dockable panels, interactive canvas
├── apps/
│   ├── optics-lab/             # Native desktop executable (macOS, Windows, Linux via eframe)
│   ├── optics-cli/             # Headless benchmark, convergence test, and batch renderer
│   └── optics-web/             # WebAssembly (WASM + WebGPU) browser deployment
└── research/                   # Reverse-engineering documentation & glass data catalogs
```

### 2.2 VectorCraft Robustness & Zero-Crash Policy
Adopt VectorCraft's workspace lints:
```toml
[workspace.lints.rust]
unsafe_code = "deny"

[workspace.lints.clippy]
dbg_macro = "warn"
unwrap_used = "deny"
expect_used = "deny"
```
* **Principle:** OpticsLab never panics. All ray tracing errors, numerical singularities, and malformed lens files return structured `Result<T, OpticsError>` enums.

---

## 3. UI/UX Design System (The "VectorCraft Standard")

### 3.1 Design Tokens (Tokenized Theme)
Following `vectorcraft-ui-egui/theme.rs`:
* **Never hard-code colors in widgets.**
* UI brightness modes: `Dark`, `MediumDark` (default), `MediumLight`, `Light`.
* Professional dark slate palette ($H=220, S=12\%, L=14\%$) with vivid cyan/amber accents for optical diagnostic highlights.

```rust
#[derive(Clone, Copy, Debug)]
pub struct OpticsTokens {
    pub panel_bg: Color32,
    pub canvas_bg: Color32,
    pub text_primary: Color32,
    pub text_dim: Color32,
    pub ray_primary: Color32,
    pub ray_reflection: Color32,
    pub ray_refraction: Color32,
    pub ray_tir: Color32,
    pub bvh_heat_cold: Color32,
    pub bvh_heat_hot: Color32,
    pub border: Color32,
    pub accent: Color32,
}
```

### 3.2 Dockable Diagnostic Panel System
OpticsLab features a professional multi-panel workspace:
1. **Interactive Central Canvas:** 3D ray path tracing viewport + 2D paraxial lens layout toggle.
2. **Left Tool Palette:** Select tool, Ray Inspector Probe tool, Lens surface tool, Light source emitter tool.
3. **Right Inspector Panel:**
   - *Tab 1: Single-Ray Inspector* (Per-bounce Snell/Fresnel parameters, trajectory table).
   - *Tab 2: Lens Prescription Table* (Surface radius, thickness, glass dropdown, semi-diameter).
   - *Tab 3: Glass Catalog Browser* (Search Schott/Ohara by refractive index $n_d$ or Abbe $V_d$).
4. **Bottom Analytics Dock:**
   - *Tab 1: Convergence & Noise Monitor* (RMSE curve, mean variance).
   - *Tab 2: Spectral Power Distribution (SPD)* (380-700 nm histogram).
   - *Tab 3: BVH Traversal Cost Heatmap* (Shader traversal counters, AABB hierarchy viewer).

---

## 4. MCP Agent Integration
Mirroring VectorCraft's `crates/mcp`, OpticsLab can expose an internal MCP server:
- AI agents (like Antigravity) can inspect the current optical system, query aberration metrics, optimize lens curvatures to minimize spot diagram RMS radius, and run automated diagnostic passes.
