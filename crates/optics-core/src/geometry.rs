//! Non-sequential triangle geometry, Möller–Trumbore intersection, and refractive media traits.

use crate::error::OpticsError;
use crate::types::{HitRecord, Ray, Vec3};

/// Trait defining medium refractive index as a function of wavelength in nanometers.
///
/// Implemented by transparent media (e.g. Schott optical glasses, water, air).
/// Kept inside `optics-core` to eliminate cyclic dependencies with higher-level catalog crates.
pub trait RefractiveIndex: Send + Sync {
    /// Returns the absolute refractive index $n$ at the specified wavelength in nanometers.
    fn n(&self, wavelength_nm: f64) -> f64;
}

/// Constant index material (e.g. idealized dielectric, water at fixed wavelength, or air).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConstantIndex(pub f64);

impl RefractiveIndex for ConstantIndex {
    #[inline]
    fn n(&self, _wavelength_nm: f64) -> f64 {
        self.0
    }
}

/// Idealized air medium with refractive index $n = 1.0000$ across all wavelengths.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Air;

impl RefractiveIndex for Air {
    #[inline]
    fn n(&self, _wavelength_nm: f64) -> f64 {
        1.0
    }
}

use serde::{Deserialize, Serialize};

/// A 3D planar triangle with refractive media boundary specifications.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Triangle {
    /// First vertex coordinate in millimeters.
    pub v0: Vec3,
    /// Second vertex coordinate in millimeters.
    pub v1: Vec3,
    /// Third vertex coordinate in millimeters.
    pub v2: Vec3,
    /// Normalized surface normal pointing according to right-handed winding order $(v_1 - v_0) \times (v_2 - v_0)$.
    pub normal: Vec3,
    /// Identifier or index of the surface within the scene.
    pub surface_index: usize,
    /// Medium index inside the volume enclosed behind the triangle normal.
    pub medium_inside: usize,
    /// Medium index in front of the triangle normal.
    pub medium_outside: usize,
}

impl Triangle {
    /// Creates a new triangle, computing its analytical unit normal from vertices.
    ///
    /// # Errors
    /// Returns [`OpticsError::NumericalSingularity`] if vertices are degenerate or collinear.
    pub fn new(
        v0: Vec3,
        v1: Vec3,
        v2: Vec3,
        surface_index: usize,
        medium_inside: usize,
        medium_outside: usize,
    ) -> Result<Self, OpticsError> {
        let edge1 = v1 - v0;
        let edge2 = v2 - v0;
        let cross = edge1.cross(&edge2);
        let norm_sq = cross.norm_squared();

        if norm_sq < 1e-24 || !norm_sq.is_finite() {
            return Err(OpticsError::NumericalSingularity(
                "Degenerate or collinear vertices for Triangle".to_string(),
            ));
        }

        let normal = cross / norm_sq.sqrt();
        Ok(Self {
            v0,
            v1,
            v2,
            normal,
            surface_index,
            medium_inside,
            medium_outside,
        })
    }

    /// Performs exact ray-triangle intersection via the Möller–Trumbore algorithm.
    ///
    /// Returns `Some(HitRecord)` if an intersection occurs within $[t_{\min}, t_{\max}]$.
    #[must_use]
    pub fn intersect(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord> {
        let edge1 = self.v1 - self.v0;
        let edge2 = self.v2 - self.v0;
        let pvec = ray.direction.cross(&edge2);
        let det = edge1.dot(&pvec);

        // Ray is parallel or near-parallel to triangle plane
        if det.abs() < 1e-12 {
            return None;
        }

        let inv_det = 1.0 / det;
        let tvec = ray.origin - self.v0;
        let u = tvec.dot(&pvec) * inv_det;
        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let qvec = tvec.cross(&edge1);
        let v = ray.direction.dot(&qvec) * inv_det;
        if v < 0.0 || (u + v) > 1.0 {
            return None;
        }

        let t = edge2.dot(&qvec) * inv_det;
        if t < t_min || t > t_max {
            return None;
        }

        let point = ray.at(t);

        // Normal points toward the incident medium (against the incident ray)
        let is_front_face = ray.direction.dot(&self.normal) <= 0.0;
        let normal = if is_front_face {
            self.normal
        } else {
            -self.normal
        };

        Some(HitRecord {
            t,
            point,
            normal,
            is_front_face,
            surface_index: self.surface_index,
        })
    }
}
