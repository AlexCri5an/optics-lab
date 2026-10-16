# OpticsLab Data Sources & Licensing Audit

This document records the provenance, mathematical reference constants, legal status, and attribution requirements for all bundled optical datasets, glass dispersion catalogs, reference spectral lines, atmospheric models, and optical prescriptions included in OpticsLab.

---

## 1. Summary of Bundled Data Assets

| Asset Path | Records / Scope | Origin / Attribution | License / Legal Status |
| :--- | :--- | :--- | :--- |
| `crates/optics-glass/data/optical_glasses.json` | 646 optical glass entries (Schott, Ohara, CDGM, Hoya, AMI, Corning) | Public manufacturer glass datasheets and [refractiveindex.info](https://refractiveindex.info) (curated by Mikhail Polyanskiy) | **Factual scientific constants** / [CC0 1.0 Universal](https://creativecommons.org/publicdomain/zero/1.0/) |
| `crates/optics-zemax/src/presets.rs` | 3 canonical optical prescriptions (Cooke Triplet 50mm F/4.5, Double Gauss 50mm F/2.0, Plano-Convex 100mm) | Classical optical literature (Taylor 1893, Kingslake 1978, Smith 2000) | **Public Domain** |
| `tools/golden/*.json` | Independent golden benchmark reference datasets (Cooke Triplet, Double Gauss, Plano-Convex) | Warren J. Smith (*Modern Optical Engineering* 4th ed.), Arthur Cox, MIL-HDBK-141 | **Scientific Literature Reference** |

---

## 2. Standard Spectral Reference Lines

OpticsLab uses the standard ISO / IUPAC Fraunhofer spectral emission line wavelengths (vacuum values in nanometers):

| Designation | Element / Source | Wavelength $\lambda$ (nm) | Region / Description |
| :--- | :--- | :--- | :--- |
| **$t$** | Hg (Mercury) | 1013.98 | Near-Infrared |
| **$s$** | Cs (Cesium) | 852.11 | Near-Infrared |
| **$r$** | Rb (Rubidium) | 706.52 | Red |
| **$C$** | H (Hydrogen) | 656.27 | Red (Fraunhofer $C$-line) |
| **$C'$** | Cd (Cadmium) | 643.85 | Red |
| **$D$** | Na (Sodium) | 589.29 | Amber / Yellow doublet mean |
| **$d$** | He (Helium) | 587.56 | Yellow (Primary optical reference line) |
| **$e$** | Hg (Mercury) | 546.07 | Green (European standard $e$-line) |
| **$F$** | H (Hydrogen) | 486.13 | Cyan / Blue (Fraunhofer $F$-line) |
| **$F'$** | Cd (Cadmium) | 479.99 | Blue |
| **$g$** | Hg (Mercury) | 435.84 | Blue |
| **$h$** | Hg (Mercury) | 404.66 | Violet |
| **$i$** | Hg (Mercury) | 365.01 | Ultraviolet |

- **Abbe Number ($V_d$)**:
  $$V_d = \frac{n_d - 1}{n_F - n_C}$$
- **European Abbe Number ($V_e$)**:
  $$V_e = \frac{n_e - 1}{n_{F'} - n_{C'}}$$

---

## 3. Atmospheric Reference Standards (`optics-spectral::thermal`)

Standard atmospheric conditions implemented in `edlen_air_standard_conditions` follow the revised Edlén formula by K. P. Birch and M. J. Downs:
- **Reference Temperature**: $T_0 = 15^\circ\text{C}$ (or $20^\circ\text{C}$ laboratory reference)
- **Reference Atmospheric Pressure**: $P_0 = 101325\,\text{Pa}$ ($101.325\,\text{kPa}$)
- **Relative Humidity**: $0\%$ dry air (with optional water vapor partial pressure correction)
- **Carbon Dioxide Concentration**: $450\,\text{ppm}$
- **Sources**:
  - K. P. Birch and M. J. Downs, "An updated Edlén equation for the refractive index of air," *Metrologia* 30(3), 155–162 (1993).
  - K. P. Birch and M. J. Downs, "Correction to the updated Edlén equation for the refractive index of air," *Metrologia* 31(4), 315–316 (1994).

---

## 4. Colorimetric Standards (`optics-spectral::color`)

- **Observer**: CIE 1931 standard 2-degree observer color-matching functions $\bar{x}(\lambda), \bar{y}(\lambda), \bar{z}(\lambda)$ tabulated from 380 nm to 780 nm at 5 nm intervals.
- **Illuminant**: CIE Standard Illuminant D65 representing daylight phase with correlated color temperature $T_c \approx 6504\,\text{K}$.
- **Color Space Conversion**: CIE $XYZ \to$ Linear sRGB $\to$ Gamma-corrected sRGB ($IEC\ 61966\text{-}2\text{-}1:1999$).

---

## 5. Independent Golden Benchmark Datasets (`tools/golden/`)

The golden reference datasets benchmark the entire computation pipeline against authoritative optical engineering publications:
1. `cooke_triplet_golden.json`:
   - Warren J. Smith, *Modern Optical Engineering: The Design of Optical Systems* (4th ed.), McGraw-Hill, New York (2007), Chapter 13.
   - MIL-HDBK-141: *Optical Design*, Department of Defense Handbook, Section 10.
2. `double_gauss_golden.json`:
   - Arthur Cox, *A System of Optical Design*, Focal Press, London (1964), Chapter 6.
   - Rudolf Kingslake, *A History of the Photographic Lens*, Academic Press, San Diego (1989).
3. `plano_convex_golden.json`:
   - Analytical closed-form thick lens formulas derived from Maxwell's boundary conditions and Gauss paraxial optics.

---

## 6. Legal & Attribution Audit

- **Scientific Facts**: Under United States copyright law (*Feist Publications, Inc. v. Rural Telephone Service Co.*, 499 U.S. 340 (1991)) and European database directives, raw scientific constants, mathematical coefficients, and physical measurements are facts devoid of copyrightable authorship.
- **Attribution**: Manufacturer names (Schott, Ohara, CDGM, Hoya) and reference authors are cited strictly for technical provenance and identification.
