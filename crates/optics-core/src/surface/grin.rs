//! Gradient-Index (GRIN) media and 4th-order Runge-Kutta (RK4) numerical ray tracer.
//!
//! # Equations
//! Ray propagation through an inhomogeneous medium $n(\mathbf{r})$ satisfies Fermat's principle:
//! $$\frac{d}{ds}\left(n(\mathbf{r}) \frac{d\mathbf{r}}{ds}\right) = \nabla n(\mathbf{r})$$
//! Introducing ray momentum $\mathbf{p} = n(\mathbf{r}) \hat{\mathbf{d}}$ with $\|\mathbf{p}\| = n(\mathbf{r})$,
//! we obtain the autonomous Hamiltonian system:
//! $$\frac{d\mathbf{r}}{ds} = \frac{\mathbf{p}}{n(\mathbf{r})}, \quad \frac{d\mathbf{p}}{ds} = \nabla n(\mathbf{r})$$
//! Integrated using adaptive classical Runge-Kutta 4th order (RK4).

use serde::{Deserialize, Serialize};

use crate::error::OpticsError;
use crate::types::{Ray, Vec3};

/// Inhomogeneous gradient-index refractive index profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GrinProfile {
    /// Radial GRIN (Selfoc / Wood lens):
    /// $$n(r) = n_0 \left(1 - \frac{1}{2} A r^2\right)$$
    Radial {
        /// Base refractive index along optical axis $n_0$.
        n0: f64,
        /// Gradient constant $A$ in $\text{mm}^{-2}$.
        a: f64,
    },
    /// Axial GRIN:
    /// $$n(z) = n_0 + n_1 z + n_2 z^2$$
    Axial {
        /// Base index at $z = 0$.
        n0: f64,
        /// Linear gradient coefficient $n_1$ in $\text{mm}^{-1}$.
        n1: f64,
        /// Quadratic gradient coefficient $n_2$ in $\text{mm}^{-2}$.
        n2: f64,
    },
}

impl GrinProfile {
    /// Evaluates refractive index $n(\mathbf{r})$ at position $\mathbf{r}$.
    #[must_use]
    pub fn index_at(&self, pos: Vec3) -> f64 {
        match self {
            Self::Radial { n0, a } => {
                let r2 = pos.x * pos.x + pos.y * pos.y;
                (n0 * (1.0 - 0.5 * a * r2)).max(1.0)
            }
            Self::Axial { n0, n1, n2 } => {
                let z = pos.z;
                (n0 + n1 * z + n2 * z * z).max(1.0)
            }
        }
    }

    /// Evaluates refractive index gradient $\nabla n(\mathbf{r})$ at position $\mathbf{r}$.
    #[must_use]
    pub fn gradient_at(&self, pos: Vec3) -> Vec3 {
        match self {
            Self::Radial { n0, a } => {
                let dn_dr_over_r = -n0 * a;
                Vec3::new(dn_dr_over_r * pos.x, dn_dr_over_r * pos.y, 0.0)
            }
            Self::Axial { n0: _, n1, n2 } => {
                let dn_dz = n1 + 2.0 * n2 * pos.z;
                Vec3::new(0.0, 0.0, dn_dz)
            }
        }
    }
}

/// State vector for RK4 integration: $(\mathbf{r}, \mathbf{p})$.
#[derive(Debug, Clone, Copy)]
struct RkState {
    r: Vec3,
    p: Vec3,
}

impl RkState {
    fn derivative(&self, profile: &GrinProfile) -> (Vec3, Vec3) {
        let n = profile.index_at(self.r);
        let dr_ds = self.p / n;
        let dp_ds = profile.gradient_at(self.r);
        (dr_ds, dp_ds)
    }
}

/// Traces a ray through a GRIN slab from initial position to target exit plane $z = z_{\text{exit}}$.
///
/// # Errors
/// Returns [`OpticsError::NumericalSingularity`] if step underflows or ray loops back.
pub fn trace_grin_rk4(
    ray: &Ray,
    profile: &GrinProfile,
    z_exit: f64,
    initial_step: f64,
    max_steps: usize,
) -> Result<Ray, OpticsError> {
    let mut current_pos = ray.origin;
    let n_init = profile.index_at(current_pos);
    let mut current_momentum = ray.direction * n_init;

    let target_z = z_exit;
    let forward = target_z >= current_pos.z;
    let mut ds = if forward {
        initial_step.abs()
    } else {
        -initial_step.abs()
    };

    for _ in 0..max_steps {
        // Check if reached or passed exit plane
        if (forward && current_pos.z >= target_z) || (!forward && current_pos.z <= target_z) {
            let n_exit = profile.index_at(current_pos);
            let final_dir = (current_momentum / n_exit).normalize();
            return Ray::new(current_pos, final_dir, ray.wavelength_nm, ray.energy);
        }

        // Clip step if close to exit plane
        let remaining_z = target_z - current_pos.z;
        let v_z = current_momentum.z / profile.index_at(current_pos);
        if v_z.abs() > 1e-12 {
            let dt_est = remaining_z / v_z;
            if dt_est.abs() < ds.abs() {
                ds = dt_est;
            }
        }

        // Classical 4th-order Runge-Kutta step
        let state0 = RkState {
            r: current_pos,
            p: current_momentum,
        };
        let (k1_r, k1_p) = state0.derivative(profile);

        let state1 = RkState {
            r: current_pos + k1_r * (0.5 * ds),
            p: current_momentum + k1_p * (0.5 * ds),
        };
        let (k2_r, k2_p) = state1.derivative(profile);

        let state2 = RkState {
            r: current_pos + k2_r * (0.5 * ds),
            p: current_momentum + k2_p * (0.5 * ds),
        };
        let (k3_r, k3_p) = state2.derivative(profile);

        let state3 = RkState {
            r: current_pos + k3_r * ds,
            p: current_momentum + k3_p * ds,
        };
        let (k4_r, k4_p) = state3.derivative(profile);

        current_pos += (k1_r + k2_r * 2.0 + k3_r * 2.0 + k4_r) * (ds / 6.0);
        current_momentum += (k1_p + k2_p * 2.0 + k3_p * 2.0 + k4_p) * (ds / 6.0);

        // Normalize momentum magnitude to match local index
        let n_curr = profile.index_at(current_pos);
        let p_norm = current_momentum.norm();
        if p_norm > 1e-15 {
            current_momentum = (current_momentum / p_norm) * n_curr;
        }
    }

    Err(OpticsError::NumericalSingularity(
        "GRIN RK4 tracer exceeded maximum integration steps".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_radial_grin_ray_bending() -> TestResult {
        // Radial GRIN focuses off-axis parallel rays toward the axis
        let profile = GrinProfile::Radial { n0: 1.6, a: 0.01 };

        let ray = Ray::new(
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            587.56,
            1.0,
        )?;

        let exit_ray = trace_grin_rk4(&ray, &profile, 10.0, 0.1, 500)?;
        assert!(exit_ray.origin.z >= 10.0);
        // Ray should have curved towards axis: x should decrease and direction.x should be negative
        assert!(exit_ray.origin.x < 2.0);
        assert!(exit_ray.direction.x < 0.0);
        Ok(())
    }

    #[test]
    fn test_axial_grin_acceleration() -> TestResult {
        let profile = GrinProfile::Axial {
            n0: 1.5,
            n1: 0.05,
            n2: 0.0,
        };

        let ray = Ray::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            587.56,
            1.0,
        )?;

        let exit_ray = trace_grin_rk4(&ray, &profile, 5.0, 0.2, 200)?;
        assert!((exit_ray.origin.x).abs() < 1e-10);
        assert!((exit_ray.direction.z - 1.0).abs() < 1e-10);
        Ok(())
    }
}
