# Reverse Engineering Document 02: Spectral Dispersion, Sellmeier Equations & Glass Catalogs

## 1. Overview & Inspiration
Natural chromatic dispersion (prisms, rainbows, chromatic aberration in camera lenses) occurs because the refractive index $n$ is a function of wavelength $\lambda$.

Inspirations reverse-engineered:
- **RefractiveIndex.info & Optiland Database:** Comprehensive dispersion formulas and coefficient sets.
- **pbrt-v4 & Mitsuba 3:** Hero-wavelength spectral sampling, CIE 1931 XYZ observer curves.
- **Zemax OpticStudio:** Schott, Ohara, and CDGM glass catalogs and Abbe number representations.

---

## 2. Mathematical Dispersion Models

### 2.1 The Standard Sellmeier Equation (Formula 1 & 2)
For optical glasses (e.g. Schott N-BK7, SF11, F2), the Sellmeier equation accurately models the refractive index over the UV-Vis-NIR spectrum (0.3 to 2.5 µm):

$$n^2(\lambda) = 1 + \sum_{i=1}^3 \frac{B_i \lambda^2}{\lambda^2 - C_i}$$

where:
- $\lambda$ is in micrometers ($\mu\text{m}$).
- $B_1, B_2, B_3$ are dimensionless oscillator strengths.
- $C_1, C_2, C_3$ are resonance wavelength squares ($\mu\text{m}^2$).

#### Concrete Coefficients for Key Optical Glasses:
| Glass Type | Manufacturer | $B_1$ | $B_2$ | $B_3$ | $C_1$ | $C_2$ | $C_3$ | $n_d$ (587.6 nm) | Abbe $V_d$ |
|------------|--------------|-------|-------|-------|-------|-------|-------|------------------|------------|
| **N-BK7**  | Schott       | 1.03961212 | 0.231792344 | 1.01046945 | 0.00600069867 | 0.0200179144 | 103.560653 | 1.5168 | 64.17 |
| **F2**     | Schott       | 1.34533359 | 0.209073176 | 0.93735716 | 0.00997743871 | 0.0470450767 | 111.886764 | 1.6200 | 36.37 |
| **SF11**   | Schott       | 1.73759695 | 0.313747346 | 1.89878101 | 0.0131887070  | 0.0623068142 | 155.23629  | 1.7847 | 25.76 |
| **Fused Silica** | Corning  | 0.6961663  | 0.4079426   | 0.8974794  | 0.004679148   | 0.01351206   | 97.9340025 | 1.4585 | 67.82 |

### 2.2 Cauchy's Empirical Formula (Fast Real-time Fallback)
For lightweight GPU compute shaders:
$$n(\lambda) = A + \frac{B}{\lambda^2} + \frac{C}{\lambda^4}$$

### 2.3 Abbe Number (Dispersion Measure)
$$V_d = \frac{n_d - 1}{n_F - n_C}$$
where standard Fraunhofer spectral lines are:
- $n_d$: Helium d-line ($587.56\,\text{nm}$)
- $n_F$: Hydrogen F-line ($486.13\,\text{nm}$)
- $n_C$: Hydrogen C-line ($656.27\,\text{nm}$)

High $V_d$ ($>50$) implies low dispersion (crown glass); low $V_d$ ($<50$) implies high dispersion (flint glass).

---

## 3. Hero-Wavelength Spectral Path Tracing

To eliminate chromatic noise in real-time path tracing:
1. Sample a random "hero" wavelength $\lambda_{\text{hero}} \sim U[380, 700]\,\text{nm}$.
2. Generate $N=4$ equidistant correlated wavelengths:
   $$\lambda_k = \left( \lambda_{\text{hero}} + k \cdot \frac{700 - 380}{N} \right) \pmod{320} + 380, \quad k \in \{0, 1, 2, 3\}$$
3. Guide the spatial path of the ray using $\lambda_{\text{hero}}$.
4. Evaluate the weights and throughputs for all $N$ wavelengths simultaneously along the path.
5. Integrate spectral energy to CIE XYZ using the CIE 1931 standard observer matching curves:
   $$X = \int_{380}^{700} L(\lambda) \bar{x}(\lambda) d\lambda$$
   $$Y = \int_{380}^{700} L(\lambda) \bar{y}(\lambda) d\lambda$$
   $$Z = \int_{380}^{700} L(\lambda) \bar{z}(\lambda) d\lambda$$
6. Convert XYZ to linear sRGB via matrix multiplication:
   $$\begin{bmatrix} R \\ G \\ B \end{bmatrix} = \begin{bmatrix} 3.2404542 & -1.5371385 & -0.4985314 \\ -0.9692660 & 1.8760108 & 0.0415560 \\ 0.0556434 & -0.2040259 & 1.0572252 \end{bmatrix} \begin{bmatrix} X \\ Y \\ Z \end{bmatrix}$$

---

## 4. Rust Data Architecture for Glass Catalog

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DispersionModel {
    Sellmeier1 { b: [f32; 3], c: [f32; 3] },
    Cauchy { a: f32, b: f32, c: f32 },
    Constant(f32),
}

impl DispersionModel {
    #[inline]
    pub fn refractive_index(&self, wavelength_nm: f32) -> f32 {
        let wl_um = wavelength_nm * 1e-3;
        match self {
            DispersionModel::Sellmeier1 { b, c } => {
                let wl_sq = wl_um * wl_um;
                let n_sq = 1.0 
                    + (b[0] * wl_sq) / (wl_sq - c[0])
                    + (b[1] * wl_sq) / (wl_sq - c[1])
                    + (b[2] * wl_sq) / (wl_sq - c[2]);
                n_sq.max(1.0).sqrt()
            }
            DispersionModel::Cauchy { a, b, c } => {
                let wl_sq = wl_um * wl_um;
                a + b / wl_sq + c / (wl_sq * wl_sq)
            }
            DispersionModel::Constant(n) => *n,
        }
    }
}
```
