# Asset Attribution & Inventory

Every non-code asset in this repository (branding icons, packaging bundles, scientific glass data, test benchmark fixtures) is listed below with its author, source, licence, and purpose.

**Clean-Room Policy:** OpticsLab contains **no proprietary iconography, decompiled assets, confidential catalogs, or commercial software files**. Every asset is either original work created for OpticsLab by the project owner/contributors, or factual scientific data, or third-party open-source material under permissive licensing (MIT, Apache-2.0, ISC, OFL-1.1, CC0).

---

## 1. Application Branding & App Icons

| Asset | Author | Source | Licence | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `assets/icon.png` | Alexandru Crisan (Project Owner) | Original artwork created for OpticsLab | MIT OR Apache-2.0 | 1024×1024 master icon; features optical lens 'O' and refracted laser beam 'L', standard 824×824 macOS squircle with drop shadow and alpha transparency |
| `assets/icon-256.png` | Alexandru Crisan (Project Owner) | Downscaled from `assets/icon.png` | MIT OR Apache-2.0 | 256×256 window icon embedded into the desktop executable (`apps/optics-desktop/src/main.rs`) via `include_bytes!` |
| `assets/OpticsLab.icns` | Alexandru Crisan (Project Owner) | Generated from `assets/icon.png` via Apple `iconutil` | MIT OR Apache-2.0 | Apple Retina Multi-Resolution ICNS bundle (16×16 to 1024×1024@2x) |
| `packaging/macos/OpticsLab.icns` | Alexandru Crisan (Project Owner) | Replica of `assets/OpticsLab.icns` | MIT OR Apache-2.0 | Bundled into `OpticsLab.app/Contents/Resources/` by `packaging/macos/package.sh` during DMG installer creation |

---

## 2. Scientific Data & Glass Catalogs

| Asset | Author / Origin | Source | Licence / Legal Status | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `crates/optics-glass/data/optical_glasses.json` | Optical Glass Foundries (Schott AG, Ohara Corp, CDGM Glass Co., Hoya Corp, Hikari, Sumita) & public domain refractive data | Public manufacturer technical datasheets | **Public Factual Scientific Data** (Physical constants of nature / Free to use for academic & engineering simulation) | Factual dispersion coefficients (Sellmeier 1 & 2, Cauchy, polynomial constants) and catalog measurements for 646 optical materials |

---

## 3. Test Verification Fixtures & Golden Benchmarks

| Asset | Author | Source | Licence | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `crates/optics-formats/tests/golden/cooke_triplet_golden.json` | OpticsLab Contributors | Computed from classical H. Dennis Taylor 1893 public domain prescription | MIT OR Apache-2.0 | Verified golden paraxial, Seidel, and ray-aiming reference benchmarks for regression tests |
| `crates/optics-formats/tests/golden/double_gauss_golden.json` | OpticsLab Contributors | Computed from classical Alvan G. Clark 1888 public domain symmetric photographic lens | MIT OR Apache-2.0 | Verified golden reference benchmarks for regression tests |
| `crates/optics-formats/tests/golden/plano_convex_golden.json` | OpticsLab Contributors | Computed from analytical paraxial lens maker formula | MIT OR Apache-2.0 | Singlet baseline verification fixture |

---

## 4. UI Vector Icons (Generated in Code)

| Component | Upstream Project | Source | Licence | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `crates/optics-ui/src/icons.rs` | [Lucide Icons](https://lucide.dev) | Upstream Lucide repository | ISC License (portions MIT by Cole Bemis / Feather) | Vector icons embedded as 100% typed Bezier curve commands with zero external runtime file dependencies |

---

## 5. Typography (Embedded by UI Framework)

The desktop application uses the default typography stack provided by `egui` / `epaint`:
* **Ubuntu Light**: Ubuntu Font Licence 1.0
* **Hack**: MIT License
* **Noto Emoji**: SIL Open Font License 1.1

---

## 6. Documentation & Gallery Screenshots

| Asset | Author | Source | Licence | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `docs/images/screenshot-workbench-2d.png` | Alexandru Crisan (Project Owner) | Captured directly from OpticsLab desktop UI | MIT OR Apache-2.0 | 3419×2151 screenshot showing 2D Sequential Optical Layout (Cooke Triplet F/4.5), Lens Data Editor spreadsheet, and multi-field Spot Diagram analysis |
| `docs/images/screenshot-3d-detector.png` | Alexandru Crisan (Project Owner) | Captured directly from OpticsLab desktop UI | MIT OR Apache-2.0 | 3419×2151 screenshot showing 3D CAD raytrace layout, 646 Glass Catalog Browser, and non-sequential 2D detector irradiance heatmap drawer |
| `docs/images/screenshot-photoreal-montecarlo.png` | Alexandru Crisan (Project Owner) | Captured directly from OpticsLab desktop UI | MIT OR Apache-2.0 | 3419×2151 screenshot showing 3D Photorealistic path-traced lens assembly (SPP 128), live DLS optimizer, and Monte Carlo tolerancing yield histogram |
