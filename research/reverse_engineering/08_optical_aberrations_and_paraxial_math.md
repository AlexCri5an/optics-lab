# Reverse Engineering Document 08: Optical Aberrations, Paraxial Matrix Optics & Diagnostics

## 1. Overview & Inspiration
To elevate OpticsLab into an open-source alternative to **Zemax OpticStudio** and **CODE V**, we have reverse-engineered the analytical mathematics for first-order Gaussian paraxial optics, ray aberration fans, spot diagrams, and Seidel third-order aberrations from **Optiland**.

---

## 2. Paraxial Ray Tracing (The $y-u$ Method)

Paraxial rays travel at infinitesimal angles to the optical axis ($z$). Under small-angle approximations ($\sin\theta \approx \theta$, $\tan\theta \approx \theta$, $\cos\theta \approx 1$):

### 2.1 Refraction Equation
At a spherical surface with radius $R$, surface power $\Phi = \frac{n' - n}{R}$:
$$n' u' = n u - y \frac{n' - n}{R} = n u - y \Phi$$
$$u' = \frac{n u - y \Phi}{n'}$$

### 2.2 Transfer Equation
Propagating an axial distance $d$ through a medium with refractive index $n'$:
$$y_{k+1} = y_k + d \cdot u_k'$$

### 2.3 Cardinal Points & First-Order Properties
By tracing two fundamental paraxial rays:
1. **Marginal Ray ($y_0 = \text{EPD}/2, u_0 = 0$):** Parallel to the axis at entrance pupil radius.
2. **Chief Ray ($y_0 = 0, u_0 = \tan\theta_{\text{field}}$):** Crosses the center of the aperture stop ($y_{\text{stop}} = 0$).

From their final ray heights and angles $(y_k, u_k')$:
* **Effective Focal Length (EFL):**
  $$f_{\text{eff}} = -\frac{y_1}{u_k'}$$
* **Back Focal Length (BFL):**
  $$\text{BFL} = -\frac{y_k}{u_k'}$$
* **Image Space F-Number ($F/\#$):**
  $$F/\# = \frac{f_{\text{eff}}}{2 y_1} = \frac{1}{2 n_k' \sin\theta_{\text{marginal}}}$$
* **Entrance Pupil Diameter (EPD) & Position:**
  Traced by projecting the aperture stop backwards into object space.
* **Exit Pupil Diameter (XPD) & Position:**
  Traced by projecting the aperture stop forward into image space.

---

## 3. Geometric Spot Diagram & RMS Blur Radius

### 3.1 Pupil Sampling Distribution
Rays are launched across the circular entrance pupil $(P_x, P_y)$ such that $P_x^2 + P_y^2 \le 1$:
* **Hexapolar Sampling:** Rings of rays with $6 \times \text{ring\_index}$ points per ring for uniform area weighting.

### 3.2 Spot Centroid & RMS Radius
For $N$ traced rays hitting the sensor plane at coordinates $(x_i, y_i)$:
* **Centroid:**
  $$\bar{x} = \frac{1}{N} \sum_{i=1}^N x_i, \quad \bar{y} = \frac{1}{N} \sum_{i=1}^N y_i$$
* **RMS Spot Radius:**
  $$\text{RMS}_{\text{radius}} = \sqrt{\frac{1}{N} \sum_{i=1}^N \left[(x_i - \bar{x})^2 + (y_i - \bar{y})^2\right]}$$
* **Geometric (GEO) Maximum Radius:**
  $$\text{GEO}_{\text{radius}} = \max_{i} \sqrt{(x_i - \bar{x})^2 + (y_i - \bar{y})^2}$$
* **Diffraction Airy Disk Radius:**
  $$r_{\text{Airy}} = 1.22 \cdot \lambda \cdot F/\#$$
  *(If $\text{RMS}_{\text{radius}} < r_{\text{Airy}}$, the optical system is **diffraction-limited**).*

---

## 4. Transverse Ray Aberration Fans

Ray fans plot transverse ray aberration errors $(e_x, e_y)$ on the image plane as a function of normalized pupil coordinates:
1. **Tangential Fan:** Plot $e_y$ vs $P_y$ with $P_x = 0$.
2. **Sagittal Fan:** Plot $e_x$ vs $P_x$ with $P_y = 0$.

### Diagnostic Interpretation:
| Curve Characteristic | Dominant Aberration | Mathematical Signature |
|----------------------|---------------------|------------------------|
| **Cubic Curve ($S$-shape)** | Spherical Aberration | $e_y \propto P_y^3$ |
| **Parabolic Curve (Asymmetric)** | Coma | $e_y \propto P_y^2$ |
| **Linear Slope (Non-zero at origin)** | Defocus | $e_y \propto P_y$ |
| **Difference between Sagittal & Tangential Slopes** | Astigmatism | $\frac{d e_y}{d P_y} \ne \frac{d e_x}{d P_x}$ |
| **Offset between Wavelength Curves** | Chromatic Aberration | Longitudinal (focal shift) + Lateral (scale shift) |

---

## 5. Optical Surface Geometries & Sag Equations

### 5.1 Conic / Spherical Surface
$$z(r) = \frac{c r^2}{1 + \sqrt{1 - (1+k) c^2 r^2}}$$
where $c = 1/R$ (curvature), $r^2 = x^2 + y^2$, and $k$ is the conic constant:
* $k = 0$: Sphere
* $-1 < k < 0$: Prolate Ellipsoid
* $k = -1$: Paraboloid
* $k < -1$: Hyperboloid
* $k > 0$: Oblate Spheroid

### 5.2 Numerically Stable Ray-Conic Intersection (Citardauque Method)
The intersection with line $\vec{r}(t) = \vec{p} + t \vec{d}$ is the quadratic $a t^2 + b t + c = 0$.
To prevent catastrophic cancellation when $a \to 0$ (near-axial rays or parabolic mirrors):
$$t = \frac{-2c}{b + \text{sign}(b)\sqrt{b^2 - 4ac}}$$

### 5.3 Analytical Surface Normal
$$\vec{n} = \frac{\left( \frac{x}{\text{denom}}, \frac{y}{\text{denom}}, -1 \right)}{\sqrt{\left(\frac{x}{\text{denom}}\right)^2 + \left(\frac{y}{\text{denom}}\right)^2 + 1}}, \quad \text{denom} = R \sqrt{1 - (1+k) c^2 r^2}$$

---

## 6. Rust Data Architecture for Lens Elements

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpticalSurface {
    pub surface_index: usize,
    pub radius_mm: f64,          // infinity for flat planes
    pub conic_constant: f64,      // 0.0 for spheres
    pub thickness_mm: f64,        // distance to next vertex
    pub clear_aperture_radius_mm: f64,
    pub glass: Option<GlassMaterial>,
    pub is_aperture_stop: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpticalPrescription {
    pub name: String,
    pub entrance_pupil_diameter_mm: f64,
    pub surfaces: Vec<OpticalSurface>,
}

impl OpticalPrescription {
    pub fn calculate_paraxial_properties(&self) -> ParaxialProperties {
        // Evaluate y-u paraxial transfer
        // ...
    }

    pub fn trace_spot_diagram(&self, num_rings: usize, field_angle_deg: f64) -> SpotDiagramData {
        // Trace hexapolar pupil rays through sequential surfaces
        // ...
    }
}
```
