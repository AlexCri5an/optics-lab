# Reverse Engineering Document 04: Convergence Tracking & Spectral Analytics

## 1. Overview & Inspiration
In physical path tracing and optical diagnostics, numerical convergence determines whether simulation results (spot diagrams, irradiance profiles, lens MTF) are statistically reliable.

Inspirations reverse-engineered:
- **pbrt-v4 & LuxCoreRender:** Online sample variance estimation, RMSE computation, image reconstruction filters.
- **Optiland Diagnostics:** Field curvature, chromatic focal shift, and spot diagram intensity distributions.

---

## 2. Statistical Convergence Formulations

### 2.1 Welford's Algorithm for Online Mean & Variance
To compute running per-pixel variance without storing every individual sample $x_k$ across iterations $k=1 \dots N$:

$$M_k = M_{k-1} + \frac{x_k - M_{k-1}}{k}$$
$$S_k = S_{k-1} + (x_k - M_{k-1})(x_k - M_k)$$
$$\sigma_k^2 = \frac{S_k}{k - 1} \quad (k \ge 2)$$

Where:
- $M_k$ is the running sample mean (the current rendered pixel color).
- $\sigma_k^2$ is the sample variance.
- Standard Error of the Mean (SEM): $\text{SEM}_k = \frac{\sigma_k}{\sqrt{k}}$.

### 2.2 Root Mean Square Error (RMSE) Against Reference
When comparing against a high-sample ground truth or embedded CPU reference tracer:
$$\text{RMSE} = \sqrt{\frac{1}{W \times H} \sum_{x=0}^{W-1} \sum_{y=0}^{H-1} \|I_{\text{current}}(x, y) - I_{\text{ref}}(x, y)\|^2}$$

---

## 3. Spectral Histogram Binning

To visualize the spectral power distribution (SPD) across transparent media and dispersion fans:
- **Wavelength Range:** 380 nm to 700 nm.
- **Bin Count:** 32 uniform bins ($\Delta \lambda = 10\,\text{nm}$).
- **Energy Binning:** When a ray hits an optical detector or sensor surface:
  $$\text{bin\_index} = \left\lfloor \frac{\lambda - 380.0}{10.0} \right\rfloor$$
  $$\text{histogram}[\text{bin\_index}] += \Phi(\lambda)$$

---

## 4. UI Analytics Panel Layout (VectorCraft Style)

```
+-------------------------------------------------------------------+
| CONVERGENCE & SPECTRAL ANALYTICS                        [x] Pin   |
+-------------------------------------------------------------------+
| Samples: 2,048 spp | Elapsed: 3.42s | Convergence Rate: 99.4%    |
| Mean Variance: 0.0014 | Peak Noise: 0.0082 (Prism Exit Face)      |
+-------------------------------------------------------------------+
| RMSE Convergence Curve:                                           |
| 1.0 | *                                                           |
| 0.5 |   *                                                         |
| 0.1 |     * * * *                                                 |
| 0.0 +---------------------------------------> Time (s)            |
+-------------------------------------------------------------------+
| Spectral Distribution (Selected Region):                          |
|      [380 nm]  --- Violet                                          |
|      [450 nm]  ======== Blue                                       |
|      [530 nm]  ================ Green                              |
|      [580 nm]  ======================= Yellow (Peak: 587.6 nm)     |
|      [650 nm]  ============== Red                                  |
+-------------------------------------------------------------------+
```
