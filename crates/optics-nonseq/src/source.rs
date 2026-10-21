//! Optical ray emission sources with explicit radiometric units and spectral sampling.

use crate::error::NonSeqError;
use optics_core::{Ray, Vec3};
use serde::{Deserialize, Serialize};

/// PRNG for deterministic ray generation.
#[derive(Debug, Clone)]
pub struct Rng64 {
    state: u64,
}

impl Rng64 {
    /// Initialize with non-zero seed.
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x853c49e6748fea9b } else { seed },
        }
    }

    /// Generate next pseudo-random 64-bit unsigned integer via XorShift64*.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    /// Sample uniformly distributed floating point in $[0, 1)$.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

/// Spectral emission distribution of an optical source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SourceSpectrum {
    /// Single discrete wavelength emission.
    Monochromatic {
        /// Wavelength in nanometers.
        wavelength_nm: f64,
    },
    /// Discrete multi-wavelength emission with relative spectral weights.
    MultiWavelength {
        /// Wavelengths in nanometers.
        wavelengths_nm: Vec<f64>,
        /// Relative power weights (normalized internally).
        weights: Vec<f64>,
    },
}

impl Default for SourceSpectrum {
    fn default() -> Self {
        Self::Monochromatic { wavelength_nm: 587.56 }
    }
}

/// Geometric optical ray source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RaySource {
    /// Point emitter radiating into a directional cone.
    Point {
        /// Source origin in world space.
        position: Vec3,
        /// Central axis direction vector.
        direction: Vec3,
        /// Cone half-angle in degrees (up to 180° for full sphere).
        cone_half_angle_deg: f64,
        /// Total radiant flux in Watts.
        total_flux_watts: f64,
        /// Emission spectrum.
        spectrum: SourceSpectrum,
    },
    /// Collimated parallel ray bundle across a circular aperture.
    CollimatedBeam {
        /// Beam center origin in world space.
        center: Vec3,
        /// Propagation direction unit vector.
        direction: Vec3,
        /// Beam circular radius in millimeters.
        radius: f64,
        /// Total radiant flux in Watts.
        total_flux_watts: f64,
        /// Emission spectrum.
        spectrum: SourceSpectrum,
    },
    /// Diffuse Lambertian planar emitting disk.
    LambertianDisk {
        /// Disk center in world space.
        center: Vec3,
        /// Surface normal vector.
        normal: Vec3,
        /// Disk radius in millimeters.
        radius: f64,
        /// Total radiant flux in Watts.
        total_flux_watts: f64,
        /// Emission spectrum.
        spectrum: SourceSpectrum,
    },
    /// Divergent Gaussian beam waist.
    GaussianBeam {
        /// Beam waist center position.
        waist_position: Vec3,
        /// Central propagation direction.
        direction: Vec3,
        /// Waist $1/e^2$ intensity radius $w_0$ in millimeters.
        waist_radius_w0: f64,
        /// Divergence half-angle in degrees $\theta = \lambda / (\pi w_0)$.
        divergence_half_angle_deg: f64,
        /// Total radiant flux in Watts.
        total_flux_watts: f64,
        /// Emission spectrum.
        spectrum: SourceSpectrum,
    },
}

/// An individual non-sequential ray carrying flux, wavelength, and traversal history.
#[derive(Debug, Clone, PartialEq)]
pub struct NonSeqRay {
    /// Geometrical ray trajectory and radiant power in Watts.
    pub ray: Ray,
    /// Unique ray identifier.
    pub ray_id: usize,
    /// Generation / bounce index (0 for initial emission).
    pub bounce_count: usize,
    /// Optional initial medium stack (empty for ambient).
    pub medium_stack: Vec<usize>,
}

impl NonSeqRay {
    /// Create a new non-sequential ray with default empty medium stack.
    pub fn new(ray: Ray, ray_id: usize) -> Self {
        Self {
            ray,
            ray_id,
            bounce_count: 0,
            medium_stack: Vec::new(),
        }
    }

    /// Builder pattern: specify the initial medium stack for rays starting inside an object.
    pub fn with_medium_stack(mut self, stack: Vec<usize>) -> Self {
        self.medium_stack = stack;
        self
    }
}

impl RaySource {
    /// Sample a wavelength from the source spectrum.
    fn sample_wavelength(&self, rng: &mut Rng64) -> f64 {
        let spectrum = match self {
            RaySource::Point { spectrum, .. } => spectrum,
            RaySource::CollimatedBeam { spectrum, .. } => spectrum,
            RaySource::LambertianDisk { spectrum, .. } => spectrum,
            RaySource::GaussianBeam { spectrum, .. } => spectrum,
        };

        match *spectrum {
            SourceSpectrum::Monochromatic { wavelength_nm } => wavelength_nm,
            SourceSpectrum::MultiWavelength {
                ref wavelengths_nm,
                ref weights,
            } => {
                if wavelengths_nm.is_empty() {
                    return 587.56;
                }
                let total_w: f64 = weights.iter().sum();
                if total_w <= 0.0 {
                    return wavelengths_nm[0];
                }
                let mut pick = rng.next_f64() * total_w;
                for (i, &w) in weights.iter().enumerate() {
                    if pick <= w {
                        return wavelengths_nm[i];
                    }
                    pick -= w;
                }
                *wavelengths_nm.last().unwrap_or(&587.56)
            }
        }
    }

    /// Generate an ensemble of non-sequential rays.
    pub fn generate_rays(&self, count: usize, seed: u64) -> Result<Vec<NonSeqRay>, NonSeqError> {
        if count == 0 {
            return Ok(Vec::new());
        }

        let mut rng = Rng64::new(seed);
        let mut rays = Vec::with_capacity(count);

        match *self {
            RaySource::Point {
                position,
                direction,
                cone_half_angle_deg,
                total_flux_watts,
                ..
            } => {
                let dir_norm = direction.norm();
                if dir_norm <= 1e-15 || !dir_norm.is_finite() {
                    return Err(NonSeqError::InvalidGeometry(
                        "Degenerate point source direction".to_string(),
                    ));
                }
                let central_dir = direction / dir_norm;
                let (u_axis, v_axis) = make_orthonormal_basis(&central_dir);
                let cos_max = cone_half_angle_deg.to_radians().cos().clamp(-1.0, 1.0);
                let ray_flux = total_flux_watts / (count as f64);

                for i in 0..count {
                    let wl = self.sample_wavelength(&mut rng);
                    let z = cos_max + rng.next_f64() * (1.0 - cos_max);
                    let phi = rng.next_f64() * 2.0 * std::f64::consts::PI;
                    let sin_theta = (1.0 - z * z).max(0.0).sqrt();

                    let ray_dir =
                        (u_axis * (sin_theta * phi.cos()) + v_axis * (sin_theta * phi.sin()) + central_dir * z)
                            .normalize();

                    let ray = Ray::new(position, ray_dir, wl, ray_flux).map_err(NonSeqError::Core)?;
                    rays.push(NonSeqRay::new(ray, i));
                }
            }
            RaySource::CollimatedBeam {
                center,
                direction,
                radius,
                total_flux_watts,
                ..
            } => {
                let dir_norm = direction.norm();
                if dir_norm <= 1e-15 || !dir_norm.is_finite() {
                    return Err(NonSeqError::InvalidGeometry(
                        "Degenerate collimated beam direction".to_string(),
                    ));
                }
                let central_dir = direction / dir_norm;
                let (u_axis, v_axis) = make_orthonormal_basis(&central_dir);
                let ray_flux = total_flux_watts / (count as f64);

                for i in 0..count {
                    let wl = self.sample_wavelength(&mut rng);
                    // Uniform disk sampling via concentric or polar square-root
                    let r = radius * rng.next_f64().sqrt();
                    let theta = rng.next_f64() * 2.0 * std::f64::consts::PI;

                    let pos = center + u_axis * (r * theta.cos()) + v_axis * (r * theta.sin());
                    let ray = Ray::new(pos, central_dir, wl, ray_flux).map_err(NonSeqError::Core)?;
                    rays.push(NonSeqRay::new(ray, i));
                }
            }
            RaySource::LambertianDisk {
                center,
                normal,
                radius,
                total_flux_watts,
                ..
            } => {
                let n_norm = normal.norm();
                if n_norm <= 1e-15 || !n_norm.is_finite() {
                    return Err(NonSeqError::InvalidGeometry(
                        "Degenerate Lambertian disk normal".to_string(),
                    ));
                }
                let n = normal / n_norm;
                let (u_axis, v_axis) = make_orthonormal_basis(&n);
                let ray_flux = total_flux_watts / (count as f64);

                for i in 0..count {
                    let wl = self.sample_wavelength(&mut rng);
                    let r = radius * rng.next_f64().sqrt();
                    let phi_pos = rng.next_f64() * 2.0 * std::f64::consts::PI;
                    let pos = center + u_axis * (r * phi_pos.cos()) + v_axis * (r * phi_pos.sin());

                    // Cosine-weighted hemispherical direction sampling: p(theta) = cos(theta)/pi
                    let r_dir = rng.next_f64().sqrt();
                    let phi_dir = rng.next_f64() * 2.0 * std::f64::consts::PI;
                    let z_dir = (1.0 - r_dir * r_dir).max(0.0).sqrt();

                    let dir =
                        (u_axis * (r_dir * phi_dir.cos()) + v_axis * (r_dir * phi_dir.sin()) + n * z_dir).normalize();

                    let ray = Ray::new(pos, dir, wl, ray_flux).map_err(NonSeqError::Core)?;
                    rays.push(NonSeqRay::new(ray, i));
                }
            }
            RaySource::GaussianBeam {
                waist_position,
                direction,
                waist_radius_w0,
                divergence_half_angle_deg,
                total_flux_watts,
                ..
            } => {
                let dir_norm = direction.norm();
                if dir_norm <= 1e-15 || !dir_norm.is_finite() {
                    return Err(NonSeqError::InvalidGeometry(
                        "Degenerate Gaussian beam direction".to_string(),
                    ));
                }
                let central_dir = direction / dir_norm;
                let (u_axis, v_axis) = make_orthonormal_basis(&central_dir);
                let ray_flux = total_flux_watts / (count as f64);
                let theta_div = divergence_half_angle_deg.to_radians();

                for i in 0..count {
                    let wl = self.sample_wavelength(&mut rng);
                    // Gaussian radial position sampling: r = w0 * sqrt(-ln(u) / 2)
                    let u_pos = rng.next_f64().max(1e-15);
                    let r_pos = waist_radius_w0 * (-u_pos.ln() * 0.5).sqrt();
                    let phi_pos = rng.next_f64() * 2.0 * std::f64::consts::PI;
                    let pos = waist_position + u_axis * (r_pos * phi_pos.cos()) + v_axis * (r_pos * phi_pos.sin());

                    // Divergence angle
                    let u_angle = rng.next_f64().max(1e-15);
                    let theta = theta_div * (-u_angle.ln() * 0.5).sqrt();
                    let phi_angle = rng.next_f64() * 2.0 * std::f64::consts::PI;

                    let dir = (u_axis * (theta.sin() * phi_angle.cos())
                        + v_axis * (theta.sin() * phi_angle.sin())
                        + central_dir * theta.cos())
                    .normalize();

                    let ray = Ray::new(pos, dir, wl, ray_flux).map_err(NonSeqError::Core)?;
                    rays.push(NonSeqRay::new(ray, i));
                }
            }
        }

        Ok(rays)
    }
}

/// Generate an arbitrary orthonormal coordinate basis $(u, v)$ perpendicular to unit vector $w$.
pub fn make_orthonormal_basis(w: &Vec3) -> (Vec3, Vec3) {
    let a = if w.x.abs() > 0.9 {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        Vec3::new(1.0, 0.0, 0.0)
    };
    let v = w.cross(&a).normalize();
    let u = w.cross(&v).normalize();
    (u, v)
}
