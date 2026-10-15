# Reverse Engineering Document 05: Zemax .zmx Format Specification & Standard Lens Presets

## 1. Overview & Inspiration
Zemax OpticStudio `.zmx` files are the de facto industry standard for optical lens design specifications (surface curvatures, thicknesses, glass materials, apertures).

Inspirations reverse-engineered:
- **Optiland Zemax Parser (`fileio/zemax/reader/parser.py`):** Operand dispatch table, surface geometry conversion.
- **Classic Lens Library:** Cooke Triplet, Double Gauss, Petzval objective configurations.

---

## 2. Zemax .zmx File Syntax Specification

A `.zmx` file is an ASCII text file with lines consisting of an operand mnemonic followed by arguments:

### 2.1 Core System Operands
| Operand | Arguments | Description | Example |
|---------|-----------|-------------|---------|
| `NAME`  | `name_str` | Optical system title | `NAME Cooke Triplet F/4.5` |
| `ENPD`  | `value`    | Entrance pupil diameter (mm) | `ENPD 10.0` |
| `FNUM`  | `value`    | Image space F-number | `FNUM 4.5` |
| `WAVL`  | `wl_um weight` | Wavelength in $\mu\text{m}$ & weighting | `WAVL 0.5876 1.0` |
| `PWAV`  | `index`    | Index of primary reference wavelength | `PWAV 2` |
| `FTYP`  | `type_id`  | Field type (0=Angle, 1=Object Height) | `FTYP 0` |
| `YFLD`  | `y1 y2 y3` | Field coordinate values | `YFLD 0.0 14.0 20.0` |

### 2.2 Surface Definition Operands
Surfaces are listed in sequential order from Object (`SURF 0`) to Image plane (`SURF N`):

| Operand | Arguments | Description | Example |
|---------|-----------|-------------|---------|
| `SURF`  | `index`   | Declares beginning of surface `index` | `SURF 1` |
| `TYPE`  | `type_str` | Surface type (`STANDARD`, `EVENASPH`, etc.) | `TYPE STANDARD` |
| `CURV`  | `curvature` | Curvature $c = 1 / R$ ($R = \text{radius in mm}$) | `CURV 0.045426` |
| `DISZ`  | `thickness` | Distance to next surface vertex along Z | `DISZ 3.25` |
| `GLAS`  | `name ...`  | Glass material name from catalog | `GLAS N-BK7 0 0 ...` |
| `DIAM`  | `semi_diam` | Semi-diameter / clear aperture radius | `DIAM 12.0` |
| `CONI`  | `k`       | Conic constant $k$ ($0=\text{sphere}, -1=\text{parabola}$) | `CONI 0.0` |
| `STOP`  | (none)    | Flags this surface as the Aperture Stop | `STOP` |

---

## 3. Canonical Lens Presets for OpticsLab

### 3.1 The Cooke Triplet (3 Elements, 6 Surfaces)
Invented by H. Dennis Taylor (1893). The minimal design that eliminates all 5 primary Seidel aberrations (spherical, coma, astigmatism, field curvature, distortion) plus chromatic aberration.

```
       Crown (SK16)      Flint (F2)      Crown (SK16)
        [ Positive ]    [ Negative ]      [ Positive ]
            | |             | |               | |
Ray ===>   (   )    ---    )   (    [STOP]   (   )   ===> Focus (Image)
            | |             | |               | |
Surf:        1 2             3 4       *       5 6            7
```

#### Prescription Table:
| Surface | Type | Radius $R$ (mm) | Thickness (mm) | Glass | Semi-Diameter (mm) | Description |
|---------|------|-----------------|----------------|-------|--------------------|-------------|
| 0 (Obj) | Flat | $\infty$        | $\infty$       | Air   | ---                | Object at $\infty$ |
| 1       | Sph  | +22.01          | 3.26           | SK16  | 6.0                | Front Crown Lens |
| 2       | Sph  | -435.76         | 6.01           | Air   | 6.0                | Air Gap |
| 3       | Sph  | -22.21          | 1.00           | F2    | 4.5                | Central Flint Lens |
| 4       | Sph  | +20.29          | 4.75           | Air   | 4.5                | Aperture Stop in gap |
| 5       | Sph  | +79.68          | 2.95           | SK16  | 5.5                | Rear Crown Lens |
| 6       | Sph  | -18.40          | 42.21          | Air   | 5.5                | Back Focal Length |
| 7 (Img) | Flat | $\infty$        | 0.0            | Air   | 10.0               | Focal Plane / Sensor |

### 3.2 Double Gauss Objective (6 Elements, 4 Groups)
The foundation of modern $50\,\text{mm}$ standard SLR camera lenses.

---

## 4. Rust Zemax Parser Architecture

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ZemaxSurface {
    pub index: usize,
    pub radius_mm: f64,
    pub thickness_mm: f64,
    pub material_name: String,
    pub semi_diameter_mm: f64,
    pub conic_constant: f64,
    pub is_stop: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ZemaxOpticalSystem {
    pub name: String,
    pub entrance_pupil_diameter_mm: f64,
    pub f_number: f64,
    pub wavelengths_um: Vec<f64>,
    pub primary_wavelength_index: usize,
    pub surfaces: Vec<ZemaxSurface>,
}

impl ZemaxOpticalSystem {
    pub fn parse_zmx(content: &str) -> Result<Self, ZemaxParseError> {
        let mut system = ZemaxOpticalSystem::default();
        let mut current_surface: Option<ZemaxSurface> = None;

        for line in content.lines() {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.is_empty() { continue; }

            match tokens[0] {
                "NAME" => system.name = tokens[1..].join(" "),
                "ENPD" => system.entrance_pupil_diameter_mm = tokens[1].parse()?,
                "FNUM" => system.f_number = tokens[1].parse()?,
                "SURF" => {
                    if let Some(surf) = current_surface.take() {
                        system.surfaces.push(surf);
                    }
                    current_surface = Some(ZemaxSurface {
                        index: tokens[1].parse()?,
                        ..Default::default()
                    });
                }
                "CURV" => {
                    if let Some(ref mut s) = current_surface {
                        let c: f64 = tokens[1].parse()?;
                        s.radius_mm = if c.abs() < 1e-12 { f64::INFINITY } else { 1.0 / c };
                    }
                }
                "DISZ" => {
                    if let Some(ref mut s) = current_surface {
                        s.thickness_mm = tokens[1].parse()?;
                    }
                }
                "GLAS" => {
                    if let Some(ref mut s) = current_surface {
                        s.material_name = tokens[1].to_string();
                    }
                }
                "DIAM" => {
                    if let Some(ref mut s) = current_surface {
                        s.semi_diameter_mm = tokens[1].parse()?;
                    }
                }
                "STOP" => {
                    if let Some(ref mut s) = current_surface {
                        s.is_stop = true;
                    }
                }
                _ => {}
            }
        }
        if let Some(surf) = current_surface {
            system.surfaces.push(surf);
        }
        Ok(system)
    }
}
```
