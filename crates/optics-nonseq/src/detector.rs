//! 2D pixelated radiometric and photometric optical detector arrays.

use crate::error::NonSeqError;
use optics_core::{Ray, Vec3};
use serde::{Deserialize, Serialize};

/// Standard CIE 1924 photopic luminous efficiency $V(\lambda)$ approximation.
pub fn photopic_luminous_efficiency(wavelength_nm: f64) -> f64 {
    if !(380.0..=780.0).contains(&wavelength_nm) {
        return 0.0;
    }
    // High-precision Gaussian sum approximation of the CIE 1924 photopic V(lambda) curve
    // Peak at lambda_0 = 555 nm with value 1.000
    let l = wavelength_nm;
    let t1 = (-0.5 * ((l - 555.0) / 45.0).powi(2)).exp();
    let t2 = 0.05 * (-0.5 * ((l - 610.0) / 30.0).powi(2)).exp();
    let val = t1 + t2;
    val.clamp(0.0, 1.0)
}

/// Convert radiant flux (Watts) at given wavelength to luminous flux (lumens).
pub fn watts_to_lumens(flux_watts: f64, wavelength_nm: f64) -> f64 {
    const KM_LUMENS_PER_WATT: f64 = 683.002;
    flux_watts * KM_LUMENS_PER_WATT * photopic_luminous_efficiency(wavelength_nm)
}

/// 2D pixelated planar detector capturing spatial irradiance and illuminance maps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Detector2D {
    /// Unique detector identifier.
    pub id: usize,
    /// Descriptive name.
    pub name: String,
    /// Detector center in world space (mm).
    pub center: Vec3,
    /// Detector face normal unit vector.
    pub normal: Vec3,
    /// Detector vertical orientation (up) unit vector.
    pub up: Vec3,
    /// Detector physical width in millimeters.
    pub width_mm: f64,
    /// Detector physical height in millimeters.
    pub height_mm: f64,
    /// Pixel grid columns.
    pub res_x: usize,
    /// Pixel grid rows.
    pub res_y: usize,
    /// Total incident radiant flux in Watts.
    pub total_flux_watts: f64,
    /// Total incident luminous flux in lumens.
    pub total_lumens: f64,
    /// Total rays absorbed / registered.
    pub total_rays_hit: usize,
    /// 2D grid of accumulated power per pixel in Watts (row-major order).
    pub pixel_flux_watts: Vec<f64>,
    /// Accumulated ray hit counts per pixel.
    pub pixel_hit_counts: Vec<u64>,
    /// Hit coordinates $(x, y)$ in detector local millimeters for centroid and spot metrics.
    pub hit_positions_mm: Vec<(f64, f64, f64)>, // (x_mm, y_mm, flux_w)
}

impl Detector2D {
    /// Create a new 2D pixelated detector.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: usize,
        name: impl Into<String>,
        center: Vec3,
        normal: Vec3,
        up: Vec3,
        width_mm: f64,
        height_mm: f64,
        res_x: usize,
        res_y: usize,
    ) -> Result<Self, NonSeqError> {
        if width_mm <= 0.0 || height_mm <= 0.0 || !width_mm.is_finite() || !height_mm.is_finite() {
            return Err(NonSeqError::InvalidGeometry(
                "Detector dimensions must be positive finite values".to_string(),
            ));
        }
        if res_x == 0 || res_y == 0 {
            return Err(NonSeqError::InvalidGeometry(
                "Detector pixel resolution must be non-zero".to_string(),
            ));
        }

        let n = normal.normalize();
        let u = up.normalize();
        if n.dot(&u).abs() > 0.9999 {
            return Err(NonSeqError::InvalidGeometry(
                "Detector normal and up vectors cannot be parallel".to_string(),
            ));
        }
        let u_ortho = (u - n * n.dot(&u)).normalize();

        let num_pixels = res_x * res_y;
        Ok(Self {
            id,
            name: name.into(),
            center,
            normal: n,
            up: u_ortho,
            width_mm,
            height_mm,
            res_x,
            res_y,
            total_flux_watts: 0.0,
            total_lumens: 0.0,
            total_rays_hit: 0,
            pixel_flux_watts: vec![0.0; num_pixels],
            pixel_hit_counts: vec![0; num_pixels],
            hit_positions_mm: Vec::new(),
        })
    }

    /// Reset all collected radiometric metrics and clear hit arrays.
    pub fn clear(&mut self) {
        self.total_flux_watts = 0.0;
        self.total_lumens = 0.0;
        self.total_rays_hit = 0;
        self.pixel_flux_watts.fill(0.0);
        self.pixel_hit_counts.fill(0);
        self.hit_positions_mm.clear();
    }

    /// Right-hand horizontal axis in detector plane.
    pub fn right(&self) -> Vec3 {
        self.up.cross(&self.normal).normalize()
    }

    /// Intersect a ray with the planar detector face.
    /// Returns `Some((t, local_x_mm, local_y_mm))` if the ray hits within rectangular bounds.
    pub fn intersect_ray(&self, ray: &Ray) -> Option<(f64, f64, f64)> {
        let denom = ray.direction.dot(&self.normal);
        // Only register rays approaching front surface (dot < 0) or double-sided
        if denom.abs() < 1e-12 {
            return None;
        }

        let t = (self.center - ray.origin).dot(&self.normal) / denom;
        if t <= 1e-9 {
            return None;
        }

        let hit_pt = ray.at(t);
        let offset = hit_pt - self.center;

        let local_x = offset.dot(&self.right());
        let local_y = offset.dot(&self.up);

        let half_w = self.width_mm * 0.5;
        let half_h = self.height_mm * 0.5;

        if local_x.abs() <= half_w && local_y.abs() <= half_h {
            Some((t, local_x, local_y))
        } else {
            None
        }
    }

    /// Record an incident ray hit on the detector.
    pub fn record_hit(&mut self, local_x: f64, local_y: f64, flux_watts: f64, wavelength_nm: f64) {
        let half_w = self.width_mm * 0.5;
        let half_h = self.height_mm * 0.5;

        let u = ((local_x + half_w) / self.width_mm).clamp(0.0, 0.999999);
        let v = ((local_y + half_h) / self.height_mm).clamp(0.0, 0.999999);

        let px = (u * self.res_x as f64) as usize;
        let py = (v * self.res_y as f64) as usize;
        let pixel_idx = py * self.res_x + px;

        let lumens = watts_to_lumens(flux_watts, wavelength_nm);

        self.total_flux_watts += flux_watts;
        self.total_lumens += lumens;
        self.total_rays_hit += 1;

        if pixel_idx < self.pixel_flux_watts.len() {
            self.pixel_flux_watts[pixel_idx] += flux_watts;
            self.pixel_hit_counts[pixel_idx] += 1;
        }

        self.hit_positions_mm.push((local_x, local_y, flux_watts));
    }

    /// Compute pixel area in square meters.
    pub fn pixel_area_m2(&self) -> f64 {
        let pixel_w_m = (self.width_mm * 1e-3) / (self.res_x as f64);
        let pixel_h_m = (self.height_mm * 1e-3) / (self.res_y as f64);
        pixel_w_m * pixel_h_m
    }

    /// Get spatial irradiance in $\text{W}/\text{m}^2$ at pixel coordinate $(x, y)$.
    pub fn irradiance_at(&self, px: usize, py: usize) -> f64 {
        if px >= self.res_x || py >= self.res_y {
            return 0.0;
        }
        let area = self.pixel_area_m2();
        if area <= 0.0 {
            return 0.0;
        }
        self.pixel_flux_watts[py * self.res_x + px] / area
    }

    /// Compute spatial centroid $(\bar{x}, \bar{y})$ in local detector coordinates (mm).
    pub fn centroid_mm(&self) -> (f64, f64) {
        if self.hit_positions_mm.is_empty() || self.total_flux_watts <= 0.0 {
            return (0.0, 0.0);
        }
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        for &(x, y, flux) in &self.hit_positions_mm {
            sum_x += x * flux;
            sum_y += y * flux;
        }
        (sum_x / self.total_flux_watts, sum_y / self.total_flux_watts)
    }

    /// Compute flux-weighted RMS spot radius around centroid in millimeters.
    pub fn rms_radius_mm(&self) -> f64 {
        if self.hit_positions_mm.is_empty() || self.total_flux_watts <= 0.0 {
            return 0.0;
        }
        let (cx, cy) = self.centroid_mm();
        let mut sum_r2 = 0.0;
        for &(x, y, flux) in &self.hit_positions_mm {
            let dx = x - cx;
            let dy = y - cy;
            sum_r2 += (dx * dx + dy * dy) * flux;
        }
        (sum_r2 / self.total_flux_watts).max(0.0).sqrt()
    }
}
