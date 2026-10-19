//! Third-order Seidel aberration coefficients and chromatic contributions.
//!
//! # Optical Foundations
//! Seidel aberration theory decomposes the primary monochromatic aberrations of an optical system
//! into five surface-by-surface contributions:
//! - **$S_I$ (Spherical Aberration / SPHA):** Aperture-dependent axial defocus variation.
//! - **$S_{II}$ (Coma / COMA):** Off-axis asymmetric magnification blur.
//! - **$S_{III}$ (Astigmatism / ASTI):** Longitudinal difference between tangential and sagittal foci.
//! - **$S_{IV}$ (Petzval Field Curvature / FCUR):** Inherent inward curvature of the best-focus surface.
//! - **$S_V$ (Distortion / DIST):** Radial cubic image deformation ($F\text{-}\tan\theta$ departure).
//!
//! Additionally, primary chromatic aberrations are computed from paraxial ray invariants:
//! - **$C_L$ (Axial Chromatic Aberration / Longitudinal Color):** Focal shift across F and C wavelengths.
//! - **$C_T$ (Transverse Chromatic Aberration / Lateral Color):** Image height magnification variation across wavelengths.
//!
//! # Analytical Formulas (Conrady / Welford Formulation)
//! For each optical surface $k$, let:
//! - Marginal ray height $y_k$ and angles $u_k$ (incident), $u_k'$ (refracted).
//! - Chief ray height $\bar{y}_k$ and angles $\bar{u}_k$ (incident), $\bar{u}_k'$ (refracted).
//! - Surface curvature $c_k = 1/R_k$ and conic constant $\kappa_k$.
//! - Refraction invariants:
//!   $$A_k = n_k (u_k + y_k c_k) = n_k' (u_k' + y_k c_k)$$
//!   $$\bar{A}_k = n_k (\bar{u}_k + \bar{y}_k c_k) = n_k' (\bar{u}_k' + \bar{y}_k c_k)$$
//! - Optical / Lagrange invariant:
//!   $$H = n_k (u_k \bar{y}_k - \bar{u}_k y_k)$$
//!
//! The surface contributions are:
//! $$S_{I, k} = A_k^2 y_k \left( \frac{u_k'}{n_k'} - \frac{u_k}{n_k} \right) - \kappa_k c_k^3 y_k^4 (n_k' - n_k)$$
//! $$S_{II, k} = A_k \bar{A}_k y_k \left( \frac{u_k'}{n_k'} - \frac{u_k}{n_k} \right) - \kappa_k c_k^3 y_k^3 \bar{y}_k (n_k' - n_k)$$
//! $$S_{III, k} = \bar{A}_k^2 y_k \left( \frac{u_k'}{n_k'} - \frac{u_k}{n_k} \right) - \kappa_k c_k^3 y_k^2 \bar{y}_k^2 (n_k' - n_k)$$
//! $$P_k = -H^2 c_k \frac{n_k' - n_k}{n_k n_k'}, \quad S_{IV, k} = P_k$$
//! $$S_{V, k} = \frac{\bar{A}_k}{A_k} [S_{III, k} + P_k] - \kappa_k c_k^3 y_k \bar{y}_k^3 (n_k' - n_k)$$
//!
//! Primary chromatic aberrations:
//! $$C_{L, k} = A_k y_k \left( \frac{\Delta n_k'}{n_k'} - \frac{\Delta n_k}{n_k} \right)$$
//! $$C_{T, k} = \bar{A}_k y_k \left( \frac{\Delta n_k'}{n_k'} - \frac{\Delta n_k}{n_k} \right)$$

use serde::{Deserialize, Serialize};

use crate::error::OpticsError;
use crate::prescription::{ResolvedPrescription, trace_paraxial_chief, trace_paraxial_marginal};

/// Surface-by-surface Seidel 3rd-order aberration contributions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceSeidel {
    /// 0-indexed surface number in prescription.
    pub surface_index: usize,
    /// Third-order spherical aberration contribution $S_I$ (mm).
    pub s1_spherical: f64,
    /// Third-order coma contribution $S_{II}$ (mm).
    pub s2_coma: f64,
    /// Third-order astigmatism contribution $S_{III}$ (mm).
    pub s3_astigmatism: f64,
    /// Petzval field curvature contribution $S_{IV}$ (mm).
    pub s4_field_curvature: f64,
    /// Third-order distortion contribution $S_V$ (mm).
    pub s5_distortion: f64,
    /// Primary axial chromatic aberration contribution $C_L$ (mm).
    pub c1_axial_color: f64,
    /// Primary transverse chromatic aberration contribution $C_T$ (mm).
    pub c2_lateral_color: f64,
}

/// Comprehensive system Seidel 3rd-order coefficients and wavefront polynomial equivalents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeidelCoefficients {
    /// Per-surface Seidel contributions.
    pub surfaces: Vec<SurfaceSeidel>,
    /// Total system spherical aberration $S_I = \sum S_{I, k}$ (mm).
    pub s1_spherical: f64,
    /// Total system coma $S_{II} = \sum S_{II, k}$ (mm).
    pub s2_coma: f64,
    /// Total system astigmatism $S_{III} = \sum S_{III, k}$ (mm).
    pub s3_astigmatism: f64,
    /// Total system field curvature $S_{IV} = \sum S_{IV, k}$ (mm).
    pub s4_field_curvature: f64,
    /// Total system distortion $S_V = \sum S_{V, k}$ (mm).
    pub s5_distortion: f64,
    /// Total system longitudinal chromatic aberration $C_L$ (mm).
    pub c1_axial_color: f64,
    /// Total system transverse chromatic aberration $C_T$ (mm).
    pub c2_lateral_color: f64,
    /// Normalized Petzval sum $P = \sum c_k \frac{n_k' - n_k}{n_k n_k'} = -S_{IV} / H^2$ ($\text{mm}^{-1}$).
    pub petzval_sum: f64,
    /// Optical Lagrange invariant $H$ (mm·rad).
    pub lagrange_invariant: f64,
    /// Primary spherical wavefront aberration coefficient $W_{040} = \frac{1}{8} S_I$ (waves or mm).
    pub w040_spherical: f64,
    /// Primary coma wavefront aberration coefficient $W_{131} = \frac{1}{2} S_{II}$ (waves or mm).
    pub w131_coma: f64,
    /// Primary astigmatism wavefront aberration coefficient $W_{222} = \frac{1}{2} S_{III}$ (waves or mm).
    pub w222_astigmatism: f64,
    /// Primary field curvature wavefront coefficient $W_{220} = \frac{1}{4} (S_{III} + S_{IV})$ (waves or mm).
    pub w220_field_curvature: f64,
    /// Primary distortion wavefront coefficient $W_{311} = \frac{1}{2} S_V$ (waves or mm).
    pub w311_distortion: f64,
    /// Primary evaluation wavelength in nanometers.
    pub primary_wavelength_nm: f64,
    /// Field angle in degrees for chief ray evaluation.
    pub field_angle_deg: f64,
}

/// Computes the third-order Seidel aberration coefficients and chromatic contributions.
///
/// # Arguments
/// - `prescription`: Resolved optical prescription with materials and geometry.
/// - `field_angle_deg`: Chief ray field angle in degrees (e.g., $0.0^\circ$ for axial, or full field).
/// - `primary_wavelength_nm`: Reference evaluation wavelength (typically $587.56\text{ nm}$).
///
/// # Returns
/// A [`SeidelCoefficients`] struct containing both surface-by-surface contributions and system totals.
///
/// # Errors
/// Returns [`OpticsError::InvalidSurface`] if the prescription has fewer than 2 surfaces or degenerate paraxial rays.
pub fn compute_seidel_coefficients(
    prescription: &ResolvedPrescription,
    field_angle_deg: f64,
    primary_wavelength_nm: f64,
) -> Result<SeidelCoefficients, OpticsError> {
    if prescription.surfaces.len() < 2 {
        return Err(OpticsError::InvalidSurface(
            "Prescription must contain at least 2 surfaces to compute Seidel coefficients"
                .to_string(),
        ));
    }

    let marginal_rays = trace_paraxial_marginal(prescription, primary_wavelength_nm)?;
    let chief_rays = trace_paraxial_chief(prescription, field_angle_deg, primary_wavelength_nm)?;

    // Reference wavelengths for dispersion: F-line (486.13 nm) and C-line (656.27 nm)
    let lambda_f = 486.13;
    let lambda_c = 656.27;

    let mut current_n = 1.0_f64; // Medium prior to surface 0 is air
    let mut current_nf = 1.0_f64;
    let mut current_nc = 1.0_f64;

    let mut current_u_marg = 0.0_f64;
    let mut current_u_chief = field_angle_deg.to_radians().tan();

    // Compute optical Lagrange invariant H at first surface
    let y0_m = marginal_rays[0].y;
    let y0_c = chief_rays[0].y;
    let h_invariant = current_n * (current_u_marg * y0_c - current_u_chief * y0_m);

    let mut surface_aberrations = Vec::with_capacity(prescription.surfaces.len());

    let mut s1_sum = 0.0_f64;
    let mut s2_sum = 0.0_f64;
    let mut s3_sum = 0.0_f64;
    let mut s4_sum = 0.0_f64;
    let mut s5_sum = 0.0_f64;
    let mut c1_sum = 0.0_f64;
    let mut c2_sum = 0.0_f64;
    let mut petzval_sum = 0.0_f64;

    for (i, surf) in prescription.surfaces.iter().enumerate() {
        let next_n = surf.material.n(primary_wavelength_nm);
        let next_nf = surf.material.n(lambda_f);
        let next_nc = surf.material.n(lambda_c);

        let delta_n = next_n - current_n;
        let delta_nf_nc = next_nf - next_nc;
        let current_delta_nf_nc = current_nf - current_nc;

        let c = surf.conic.curvature;
        let kappa = surf.conic.conic_constant;

        let y_m = marginal_rays[i].y;
        let u_m_prime = marginal_rays[i].u;

        let y_c = chief_rays[i].y;
        let u_c_prime = chief_rays[i].u;

        // Marginal and chief angles of incidence
        let i_m = current_u_marg + y_m * c;
        let i_c = current_u_chief + y_c * c;

        // Refraction invariants
        let a_m = current_n * i_m;
        let a_c = current_n * i_c;

        // Delta(u/n) across refraction
        let delta_u_over_n = (u_m_prime / next_n) - (current_u_marg / current_n);

        // Third-order Seidel surface contributions (Welford / Hopkins sign convention)
        // Aspheric conic departure delta_conic = kappa * c^3 * y^4 * delta_n
        let asph_base = kappa * c.powi(3) * delta_n;

        let s1 = -a_m * a_m * y_m * delta_u_over_n + asph_base * y_m.powi(4);
        let s2 = -a_m * a_c * y_m * delta_u_over_n + asph_base * y_m.powi(3) * y_c;
        let s3 = -a_c * a_c * y_m * delta_u_over_n + asph_base * y_m.powi(2) * y_c.powi(2);

        // Petzval curvature contribution: P_k = c_k * (n' - n) / (n * n'), S4 = -H^2 * P_k
        let petzval_k = c * delta_n / (current_n * next_n);
        let s4 = -h_invariant * h_invariant * petzval_k;
        petzval_sum += petzval_k;

        // Distortion S_V
        let s5 = if a_m.abs() > 1e-14 {
            (a_c / a_m) * (s3 + s4) + asph_base * y_m * y_c.powi(3)
        } else {
            let delta_u_c_over_n = (u_c_prime / next_n) - (current_u_chief / current_n);
            -a_c.powi(2) * y_c * delta_u_c_over_n + asph_base * y_c.powi(4)
        };

        // Primary chromatic aberrations
        let delta_dispersion_ratio = (delta_nf_nc / next_n) - (current_delta_nf_nc / current_n);
        let c1 = -a_m * y_m * delta_dispersion_ratio;
        let c2 = -a_c * y_m * delta_dispersion_ratio;

        surface_aberrations.push(SurfaceSeidel {
            surface_index: i,
            s1_spherical: s1,
            s2_coma: s2,
            s3_astigmatism: s3,
            s4_field_curvature: s4,
            s5_distortion: s5,
            c1_axial_color: c1,
            c2_lateral_color: c2,
        });

        s1_sum += s1;
        s2_sum += s2;
        s3_sum += s3;
        s4_sum += s4;
        s5_sum += s5;
        c1_sum += c1;
        c2_sum += c2;

        current_n = next_n;
        current_nf = next_nf;
        current_nc = next_nc;
        current_u_marg = u_m_prime;
        current_u_chief = u_c_prime;
    }

    Ok(SeidelCoefficients {
        surfaces: surface_aberrations,
        s1_spherical: s1_sum,
        s2_coma: s2_sum,
        s3_astigmatism: s3_sum,
        s4_field_curvature: s4_sum,
        s5_distortion: s5_sum,
        c1_axial_color: c1_sum,
        c2_lateral_color: c2_sum,
        petzval_sum,
        lagrange_invariant: h_invariant,
        w040_spherical: 0.125 * s1_sum,
        w131_coma: 0.5 * s2_sum,
        w222_astigmatism: 0.5 * s3_sum,
        w220_field_curvature: 0.25 * (s3_sum + s4_sum),
        w311_distortion: 0.5 * s5_sum,
        primary_wavelength_nm,
        field_angle_deg,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conic::ConicSurface;
    use crate::geometry::{Air, ConstantIndex};
    use crate::prescription::{ResolvedSurface, calculate_paraxial_properties};
    use std::sync::Arc;

    #[test]
    fn test_plano_convex_singlet_petzval_sum_agrees_with_analytical_formula()
    -> Result<(), Box<dyn std::error::Error>> {
        // For a thin plano-convex singlet with index n and focal length f:
        // Petzval sum = 1 / (n * f)
        let n = 1.5;
        let r1 = 50.0;
        let _r2 = f64::INFINITY;
        let d = 0.001; // thin limit

        let presc = ResolvedPrescription {
            name: "Plano-Convex Singlet".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            field_angles_deg: vec![0.0, 5.0],
            wavelengths_nm: vec![587.56],
            primary_wavelength_index: 0,
            surfaces: vec![
                ResolvedSurface {
                    surface_index: 0,
                    conic: ConicSurface::sphere(r1, 15.0)?.with_vertex_z(0.0),
                    asphere: None,
                    coordinate_break: None,
                    global_transform: None,
                    grin_profile: None,
                    thickness_mm: d,
                    material: Arc::new(ConstantIndex(n)),
                    material_name: "MockGlass".to_string(),
                    is_stop: true,
                },
                ResolvedSurface {
                    surface_index: 1,
                    conic: ConicSurface::plane(15.0)?.with_vertex_z(d),
                    asphere: None,
                    coordinate_break: None,
                    global_transform: None,
                    grin_profile: None,
                    thickness_mm: 0.0,
                    material: Arc::new(Air),
                    material_name: "AIR".to_string(),
                    is_stop: false,
                },
            ],
        };

        let paraxial = calculate_paraxial_properties(&presc, 587.56)?;
        let expected_petzval = 1.0 / (n * paraxial.efl_mm);

        let seidel = compute_seidel_coefficients(&presc, 0.0, 587.56)?;

        let rel_diff = (seidel.petzval_sum - expected_petzval).abs() / expected_petzval;
        assert!(
            rel_diff < 1e-4,
            "Petzval sum mismatch: got {}, expected {}, rel_diff = {}",
            seidel.petzval_sum,
            expected_petzval,
            rel_diff
        );
        // Also check that S1 (spherical aberration) is positive (undercorrected spherical for simple singlet)
        assert!(
            seidel.s1_spherical > 0.0,
            "Plano-convex singlet must have positive undercorrected S1"
        );
        Ok(())
    }
}
