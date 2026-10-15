//! 3D vector formulations of Snell's Law of refraction and specular reflection.
//!
//! # Vector Formulation of Snell's Law
//!
//! Given an incident unit vector $\mathbf{i}$, an interface unit normal $\mathbf{n}$
//! pointing toward the incident medium such that $\mathbf{i} \cdot \mathbf{n} \le 0$,
//! and relative index of refraction $\eta = n_1 / n_2$:
//!
//! $$\cos\theta_i = -\mathbf{i} \cdot \mathbf{n}$$
//!
//! By Snell's Law in scalar form:
//!
//! $$n_1 \sin\theta_i = n_2 \sin\theta_t \implies \sin^2\theta_t = \eta^2 (1 - \cos^2\theta_i)$$
//!
//! The discriminant governing real transmitted solutions is:
//!
//! $$k = 1 - \eta^2(1 - \cos^2\theta_i) = \cos^2\theta_t$$
//!
//! If $k < 0$, total internal reflection (TIR) occurs and no transmitted ray exists.
//! When $k \ge 0$, the transmitted unit direction is:
//!
//! $$\mathbf{t} = \eta \mathbf{i} + (\eta \cos\theta_i - \sqrt{k})\mathbf{n}$$
//!
//! # Rationale for this Formulation
//!
//! An alternate formulation found in some computer graphics literature uses double cross products:
//!
//! $$\mathbf{t} = \eta \mathbf{n} \times (-\mathbf{n} \times \mathbf{i}) - \mathbf{n} \sqrt{k}$$
//!
//! While mathematically identical in infinite precision, the standard dot-product
//! formulation chosen here requires only 3 multiplications and 2 additions to project along
//! $\mathbf{n}$, avoiding 18 multiplications and 12 subtractions across two cross products.
//! It exhibits superior numerical stability and precision near grazing incidence.

use crate::error::OpticsError;
use crate::types::Vec3;

/// Computes the specular reflection unit vector $\mathbf{r}$ for an incident direction $\mathbf{i}$
/// and surface normal $\mathbf{n}$.
///
/// # Normal Convention
/// The normal $\mathbf{n}$ is expected to point toward the incident medium ($\mathbf{i} \cdot \mathbf{n} \le 0$).
/// If a flipped normal ($\mathbf{i} \cdot \mathbf{n} > 0$) is supplied, it is automatically
/// oriented to point against the incident ray.
///
/// # Formula
/// $$\mathbf{r} = \mathbf{i} - 2(\mathbf{i} \cdot \mathbf{n})\mathbf{n}$$
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] if incident or normal vectors are non-finite
/// or zero-length.
pub fn reflect(incident: Vec3, normal: Vec3) -> Result<Vec3, OpticsError> {
    if !incident.iter().all(|x| x.is_finite()) || !normal.iter().all(|x| x.is_finite()) {
        return Err(OpticsError::NumericalSingularity(
            "Incident or normal vector contains non-finite values in reflect".to_string(),
        ));
    }

    let i_norm_sq = incident.norm_squared();
    let n_norm_sq = normal.norm_squared();
    if i_norm_sq < 1e-30 || n_norm_sq < 1e-30 {
        return Err(OpticsError::NumericalSingularity(
            "Zero or near-zero norm in reflect".to_string(),
        ));
    }

    let i = incident / i_norm_sq.sqrt();
    let mut n = normal / n_norm_sq.sqrt();

    let mut dot_in = i.dot(&n);
    if dot_in > 0.0 {
        n = -n;
        dot_in = -dot_in;
    }

    let r = i - 2.0 * dot_in * n;
    debug_assert!(
        (r.norm_squared() - 1.0).abs() < 1e-10,
        "Reflected vector must be unit length"
    );

    Ok(r)
}

/// Computes the specular refraction unit vector $\mathbf{t}$ across a dielectric boundary
/// using Snell's Law in 3D vector form.
///
/// # Arguments
/// * `incident` — Incident ray propagation unit direction.
/// * `normal` — Surface unit normal at the interface point.
/// * `n1` — Refractive index of the incident medium ($> 0$).
/// * `n2` — Refractive index of the transmitted medium ($> 0$).
///
/// # Normal Convention
/// The normal is canonicalized such that it points toward the incident medium ($\mathbf{i} \cdot \mathbf{n} \le 0$).
/// If a normal pointing into the transmitted medium is passed, it is inverted internally.
///
/// # Errors
/// * [`OpticsError::TotalInternalReflection`] if $1 - (n_1/n_2)^2(1 - \cos^2\theta_i) < 0$.
/// * [`OpticsError::NumericalSingularity`] if inputs contain NaN/inf, if vectors have zero length,
///   or if refractive indices are non-positive.
pub fn snell_refract(incident: Vec3, normal: Vec3, n1: f64, n2: f64) -> Result<Vec3, OpticsError> {
    if !incident.iter().all(|x| x.is_finite())
        || !normal.iter().all(|x| x.is_finite())
        || !n1.is_finite()
        || !n2.is_finite()
    {
        return Err(OpticsError::NumericalSingularity(
            "Non-finite inputs provided to snell_refract".to_string(),
        ));
    }

    if n1 <= 0.0 || n2 <= 0.0 {
        return Err(OpticsError::NumericalSingularity(
            "Refractive indices must be strictly positive".to_string(),
        ));
    }

    let i_norm_sq = incident.norm_squared();
    let n_norm_sq = normal.norm_squared();
    if i_norm_sq < 1e-30 || n_norm_sq < 1e-30 {
        return Err(OpticsError::NumericalSingularity(
            "Zero or near-zero norm in snell_refract".to_string(),
        ));
    }

    let i = incident / i_norm_sq.sqrt();
    let mut n = normal / n_norm_sq.sqrt();

    let mut cos_i = -i.dot(&n);
    if cos_i < 0.0 {
        n = -n;
        cos_i = -cos_i;
    }
    // Clamp to [0, 1] to avoid domain errors under slight numerical drift
    let cos_i = cos_i.clamp(0.0, 1.0);

    let eta = n1 / n2;
    let sin2_i = (1.0 - cos_i * cos_i).max(0.0);
    let k = 1.0 - eta * eta * sin2_i;

    if k < 0.0 {
        return Err(OpticsError::TotalInternalReflection);
    }

    let cos_t = k.sqrt();
    let t = eta * i + (eta * cos_i - cos_t) * n;

    debug_assert!(
        (t.norm_squared() - 1.0).abs() < 1e-10,
        "Refracted vector must be unit length"
    );

    Ok(t)
}
