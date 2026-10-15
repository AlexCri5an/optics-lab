//! Fresnel equations for dielectric media and the fast Schlick approximation.
//!
//! # Exact Fresnel Dielectric Equations
//!
//! For light crossing an interface between two non-magnetic, non-absorbing dielectric media
//! with refractive indices $n_1$ and $n_2$, the amplitude reflection coefficients for
//! perpendicular ($s$) and parallel ($p$) polarizations are given by:
//!
//! $$r_s = \frac{n_1 \cos\theta_i - n_2 \cos\theta_t}{n_1 \cos\theta_i + n_2 \cos\theta_t}$$
//!
//! $$r_p = \frac{n_2 \cos\theta_i - n_1 \cos\theta_t}{n_2 \cos\theta_i + n_1 \cos\theta_t}$$
//!
//! The corresponding power reflectances (energy fractions) are:
//!
//! $$R_s = |r_s|^2, \quad R_p = |r_p|^2$$
//!
//! In accordance with energy conservation for lossless media, power transmittances satisfy:
//!
//! $$T_s = 1 - R_s, \quad T_p = 1 - R_p$$
//!
//! For unpolarized light (the typical case in non-polarizing ray tracers and inspectors):
//!
//! $$R_{\text{total}} = \frac{R_s + R_p}{2}, \quad T_{\text{total}} = 1 - R_{\text{total}}$$

/// Evaluates exact power reflectances $(R_s, R_p)$ for a dielectric interface.
///
/// # Arguments
/// * `cos_theta_i` — Cosine of the incident angle ($\cos\theta_i \in [0, 1]$). Values outside $[0, 1]$ are clamped.
/// * `n1` — Refractive index of the incident medium ($n_1 > 0$).
/// * `n2` — Refractive index of the transmitted medium ($n_2 > 0$).
///
/// # Returns
/// A tuple `(Rs, Rp)` of power reflectances in $[0, 1]$. In case of total internal reflection,
/// `(1.0, 1.0)` is returned.
#[must_use]
pub fn fresnel_dielectric(cos_theta_i: f64, n1: f64, n2: f64) -> (f64, f64) {
    let cos_i = cos_theta_i.clamp(0.0, 1.0);
    let eta = n1 / n2;
    let sin2_t = eta * eta * (1.0 - cos_i * cos_i).max(0.0);

    // Total Internal Reflection
    if sin2_t >= 1.0 {
        return (1.0, 1.0);
    }

    let cos_t = (1.0 - sin2_t).max(0.0).sqrt();

    let rs_num = n1 * cos_i - n2 * cos_t;
    let rs_den = n1 * cos_i + n2 * cos_t;
    let rs = if rs_den.abs() < 1e-30 {
        0.0
    } else {
        (rs_num / rs_den).powi(2).clamp(0.0, 1.0)
    };

    let rp_num = n2 * cos_i - n1 * cos_t;
    let rp_den = n2 * cos_i + n1 * cos_t;
    let rp = if rp_den.abs() < 1e-30 {
        0.0
    } else {
        (rp_num / rp_den).powi(2).clamp(0.0, 1.0)
    };

    (rs, rp)
}

/// Evaluates unpolarized total reflectance and transmittance $(R_{\text{total}}, T_{\text{total}})$.
///
/// # Returns
/// A tuple `(r_total, t_total)` where $R_{\text{total}} = (R_s + R_p) / 2$ and $T_{\text{total}} = 1 - R_{\text{total}}$.
#[must_use]
pub fn fresnel_unpolarized(cos_theta_i: f64, n1: f64, n2: f64) -> (f64, f64) {
    let (rs, rp) = fresnel_dielectric(cos_theta_i, n1, n2);
    let r_total = (rs + rp) * 0.5;
    let t_total = (1.0 - r_total).clamp(0.0, 1.0);
    (r_total, t_total)
}

/// Computes Schlick's fast empirical approximation for unpolarized dielectric reflectance.
///
/// # Formula
/// $$R_0 = \left(\frac{n_1 - n_2}{n_1 + n_2}\right)^2$$
///
/// $$R(\theta) = R_0 + (1 - R_0)(1 - \cos\theta)^5$$
///
/// When $n_1 > n_2$, total internal reflection is checked first; if real refraction occurs,
/// $\cos\theta_t$ is used in place of $\cos\theta_i$.
///
/// # Accuracy & Deviations
/// Schlick's approximation matches exact unpolarized Fresnel at normal incidence ($\theta = 0$)
/// and grazing incidence ($\theta = 90^\circ$). However, it deviates in intermediate angles,
/// most notably near Brewster's angle $\theta_B = \arctan(n_2/n_1)$ where exact $R_p = 0$, causing
/// the exact unpolarized reflectance to dip lower than Schlick predicts.
#[must_use]
pub fn schlick_approx(cos_theta_i: f64, n1: f64, n2: f64) -> f64 {
    let cos_i = cos_theta_i.clamp(0.0, 1.0);
    let r0 = ((n1 - n2) / (n1 + n2)).powi(2);

    let cos_theta = if n1 <= n2 {
        cos_i
    } else {
        let eta = n1 / n2;
        let sin2_t = eta * eta * (1.0 - cos_i * cos_i).max(0.0);
        if sin2_t >= 1.0 {
            return 1.0; // Total internal reflection
        }
        (1.0 - sin2_t).max(0.0).sqrt()
    };

    let x = (1.0 - cos_theta).clamp(0.0, 1.0);
    (r0 + (1.0 - r0) * x.powi(5)).clamp(0.0, 1.0)
}
