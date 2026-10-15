# Reverse Engineering Document 01: Single-Ray Inspector & Physical Ray-Path Diagnostics

## 1. Overview & Inspiration
The **Single-Ray Inspector** is a diagnostic system allowing users to click any pixel in the 3D viewport or 2D lens cross-section to unpack and inspect the complete ray path from emitter/camera to termination.

Inspirations reverse-engineered:
- **Mitsuba 3 & pbrt-v4:** Particle & ray trajectory recording, BSDF bounce evaluation, transmittance modeling.
- **Ray Optics Simulation (Rick Tu):** Segmented step-by-step ray propagation, refractive boundary handling, ray branching at splitters.
- **Optiland:** Sequential & non-sequential ray state tracing with surface intercept data.

---

## 2. Mathematical Formulations

### 2.1 Ray Representation
A physical ray segment is parameterized as:
$$\vec{r}(t) = \vec{o} + t \cdot \vec{d}, \quad t \ge 0$$
where:
- $\vec{o} \in \mathbb{R}^3$ is the origin.
- $\vec{d} \in \mathbb{R}^3$ is the normalized unit direction vector ($|\vec{d}| = 1$).
- $\lambda \in [380, 700]\,\text{nm}$ is the ray's wavelength.
- $\vec{\Phi} \in \mathbb{R}^4$ is the spectral radiance / Stokes polarization vector $(I, Q, U, V)$.

### 2.2 Snell's Law in 3D Vector Form
At an interface between medium 1 (refractive index $n_1(\lambda)$) and medium 2 ($n_2(\lambda)$) with surface normal $\vec{n}$:
$$\eta = \frac{n_1(\lambda)}{n_2(\lambda)}$$
$$\cos\theta_i = -\vec{n} \cdot \vec{d}$$
The refracted direction $\vec{t}$ is:
$$\sin^2\theta_t = \eta^2 (1 - \cos^2\theta_i)$$
- **Total Internal Reflection (TIR):** If $\sin^2\theta_t > 1$, refraction is physically impossible; 100% of the energy is reflected:
  $$\vec{r}_{\text{refl}} = \vec{d} + 2 \cos\theta_i \vec{n}$$
- **Refracted Ray Vector:**
  $$\vec{t} = \eta \vec{d} + \left(\eta \cos\theta_i - \sqrt{1 - \sin^2\theta_t}\right) \vec{n}$$

### 2.3 Dielectric Fresnel Equations
For unpolarized light, the power reflection coefficient $R \in [0, 1]$ is:
$$R_s = \left| \frac{n_1 \cos\theta_i - n_2 \cos\theta_t}{n_1 \cos\theta_i + n_2 \cos\theta_t} \right|^2$$
$$R_p = \left| \frac{n_2 \cos\theta_i - n_1 \cos\theta_t}{n_2 \cos\theta_i + n_1 \cos\theta_t} \right|^2$$
$$R = \frac{R_s + R_p}{2}, \quad T = 1 - R$$
*(Or Schlick's approximation for real-time performance)*:
$$R(\theta) \approx R_0 + (1 - R_0)(1 - \cos\theta_i)^5, \quad R_0 = \left(\frac{n_1 - n_2}{n_1 + n_2}\right)^2$$

---

## 3. Data Structures for the Inspector

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RayBouncePoint {
    pub bounce_index: u32,
    pub position: [f32; 3],
    pub incident_dir: [f32; 3],
    pub surface_normal: [f32; 3],
    pub outgoing_dir: [f32; 3],
    pub wavelength_nm: f32,
    pub ior_in: f32,
    pub ior_out: f32,
    pub theta_incident_deg: f32,
    pub theta_transmitted_deg: f32,
    pub fresnel_reflectance: f32,
    pub fresnel_transmittance: f32,
    pub event_type: BounceEventType,
    pub surface_name: String,
    pub throughput: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BounceEventType {
    SpecularReflection,
    SpecularRefraction,
    TotalInternalReflection,
    DiffuseScatter,
    AbsorptionTermination,
    EscapedScene,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct InspectedRayTrajectory {
    pub pixel_coords: [u32; 2],
    pub primary_wavelength_nm: f32,
    pub bounces: Vec<RayBouncePoint>,
    pub total_optical_path_length: f32,
    pub final_radiance: [f32; 3],
}
```

---

## 4. Single-Ray Inspector UI Layout (VectorCraft Style)

```
+-------------------------------------------------------------------+
| SINGLE-RAY INSPECTOR                                    [x] Pin   |
+-------------------------------------------------------------------+
| Pixel: (512, 384) | Hero Wavelength: 587.6 nm (d-line)            |
| Total Bounces: 4  | Optical Path Length (OPL): 142.85 mm          |
+-------------------------------------------------------------------+
| Step | Event | Surface    | IOR  | Inc. Angle | Refl % | Trans %  |
|------|-------|------------|------|------------|--------|----------|
| 0    | EMIT  | Camera     | 1.00 | ---        | ---    | 100%     |
| 1    | REFR  | Lens 1 F   | 1.52 | 14.2 deg   | 4.2%   | 95.8%    |
| 2    | REFR  | Lens 1 B   | 1.00 | 9.8 deg    | 4.0%   | 96.0%    |
| 3    | REFL  | Prism Base | 1.62 | 48.5 (TIR) | 100%   | 0.0%     |
| 4    | TERM  | Sensor/Sky | ---  | ---        | ---    | ---      |
+-------------------------------------------------------------------+
| [ View in 3D ] [ Export Ray Path JSON ] [ Copy Snell Math ]       |
+-------------------------------------------------------------------+
```
