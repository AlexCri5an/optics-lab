//! Optical surface apertures and central/off-axis obscurations.
//!
//! Supports circular, rectangular, elliptical, annular, and general polygonal shapes
//! with independent $(x_d, y_d)$ decenters and obscuration inversion flags.

use serde::{Deserialize, Serialize};

use crate::error::OpticsError;

/// Geometric shape defining an aperture transmission boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ApertureShape {
    /// Circular clear aperture of radius $r_{\max}$.
    Circular {
        /// Outer radius in millimeters.
        radius_mm: f64,
    },
    /// Rectangular clear aperture.
    Rectangular {
        /// Half-width along $x$ in millimeters.
        half_width_mm: f64,
        /// Half-height along $y$ in millimeters.
        half_height_mm: f64,
    },
    /// Elliptical clear aperture.
    Elliptical {
        /// Semi-major/minor axis along $x$ in millimeters.
        semi_axis_x_mm: f64,
        /// Semi-major/minor axis along $y$ in millimeters.
        semi_axis_y_mm: f64,
    },
    /// Annular aperture (e.g. Cassegrain secondary obscuration or ring baffle).
    Annular {
        /// Inner obscuration radius in millimeters.
        inner_radius_mm: f64,
        /// Outer clear radius in millimeters.
        outer_radius_mm: f64,
    },
    /// General 2D polygonal aperture defined by ordered $(x, y)$ vertices.
    Polygonal {
        /// Ordered vertex list forming a closed polygon in millimeters.
        vertices: Vec<(f64, f64)>,
    },
}

/// Optical aperture specification with optional decenter and obscuration behavior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Aperture {
    /// Geometric shape of the aperture boundary.
    pub shape: ApertureShape,
    /// Decenter offset along $x$ in millimeters.
    pub decenter_x_mm: f64,
    /// Decenter offset along $y$ in millimeters.
    pub decenter_y_mm: f64,
    /// If true, this aperture acts as an obscuration/baffle (transmits outside, blocks inside).
    pub is_obscuration: bool,
}

impl Aperture {
    /// Creates a circular aperture centered on the optical axis.
    #[must_use]
    pub const fn circular(radius_mm: f64) -> Self {
        Self {
            shape: ApertureShape::Circular { radius_mm },
            decenter_x_mm: 0.0,
            decenter_y_mm: 0.0,
            is_obscuration: false,
        }
    }

    /// Creates an annular aperture (e.g. Cassegrain telescope pupil).
    #[must_use]
    pub const fn annular(inner_radius_mm: f64, outer_radius_mm: f64) -> Self {
        Self {
            shape: ApertureShape::Annular {
                inner_radius_mm,
                outer_radius_mm,
            },
            decenter_x_mm: 0.0,
            decenter_y_mm: 0.0,
            is_obscuration: false,
        }
    }

    /// Creates a rectangular aperture centered on the optical axis.
    #[must_use]
    pub const fn rectangular(half_width_mm: f64, half_height_mm: f64) -> Self {
        Self {
            shape: ApertureShape::Rectangular {
                half_width_mm,
                half_height_mm,
            },
            decenter_x_mm: 0.0,
            decenter_y_mm: 0.0,
            is_obscuration: false,
        }
    }

    /// Creates an elliptical aperture centered on the optical axis.
    #[must_use]
    pub const fn elliptical(semi_axis_x_mm: f64, semi_axis_y_mm: f64) -> Self {
        Self {
            shape: ApertureShape::Elliptical {
                semi_axis_x_mm,
                semi_axis_y_mm,
            },
            decenter_x_mm: 0.0,
            decenter_y_mm: 0.0,
            is_obscuration: false,
        }
    }

    /// Returns the maximum radial distance from the coordinate origin to the outer boundary.
    #[must_use]
    pub fn max_radial_extent(&self) -> f64 {
        let shape_extent = match &self.shape {
            ApertureShape::Circular { radius_mm } => *radius_mm,
            ApertureShape::Rectangular {
                half_width_mm,
                half_height_mm,
            } => (half_width_mm * half_width_mm + half_height_mm * half_height_mm).sqrt(),
            ApertureShape::Elliptical {
                semi_axis_x_mm,
                semi_axis_y_mm,
            } => semi_axis_x_mm.max(*semi_axis_y_mm),
            ApertureShape::Annular { outer_radius_mm, .. } => *outer_radius_mm,
            ApertureShape::Polygonal { vertices } => vertices
                .iter()
                .map(|(x, y)| (x * x + y * y).sqrt())
                .fold(0.0_f64, f64::max),
        };
        let decenter_offset =
            (self.decenter_x_mm * self.decenter_x_mm + self.decenter_y_mm * self.decenter_y_mm).sqrt();
        shape_extent + decenter_offset
    }

    /// Tests whether point $(x, y)$ in surface coordinates transmits through this aperture.
    ///
    /// # Errors
    /// Returns [`OpticsError::InvalidParameter`] if coordinates are non-finite.
    pub fn transmits(&self, x: f64, y: f64) -> Result<bool, OpticsError> {
        if !x.is_finite() || !y.is_finite() {
            return Err(OpticsError::InvalidParameter("Coordinates must be finite".to_string()));
        }

        let local_x = x - self.decenter_x_mm;
        let local_y = y - self.decenter_y_mm;

        let inside_shape = match &self.shape {
            ApertureShape::Circular { radius_mm } => local_x * local_x + local_y * local_y <= radius_mm * radius_mm,
            ApertureShape::Rectangular {
                half_width_mm,
                half_height_mm,
            } => local_x.abs() <= *half_width_mm && local_y.abs() <= *half_height_mm,
            ApertureShape::Elliptical {
                semi_axis_x_mm,
                semi_axis_y_mm,
            } => {
                let term_x = local_x / semi_axis_x_mm;
                let term_y = local_y / semi_axis_y_mm;
                term_x * term_x + term_y * term_y <= 1.0
            }
            ApertureShape::Annular {
                inner_radius_mm,
                outer_radius_mm,
            } => {
                let r2 = local_x * local_x + local_y * local_y;
                r2 >= inner_radius_mm * inner_radius_mm && r2 <= outer_radius_mm * outer_radius_mm
            }
            ApertureShape::Polygonal { vertices } => point_in_polygon(local_x, local_y, vertices),
        };

        if self.is_obscuration {
            Ok(!inside_shape)
        } else {
            Ok(inside_shape)
        }
    }
}

/// Standard ray-casting parity algorithm for 2D point-in-polygon containment.
fn point_in_polygon(x: f64, y: f64, vertices: &[(f64, f64)]) -> bool {
    let n = vertices.len();
    if n < 3 {
        return false;
    }

    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = vertices[i];
        let (xj, yj) = vertices[j];

        let intersect = ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi);
        if intersect {
            inside = !inside;
        }
        j = i;
    }

    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_circular_and_annular_aperture() -> TestResult {
        let circ = Aperture::circular(10.0);
        assert!(circ.transmits(0.0, 0.0)?);
        assert!(circ.transmits(6.0, 8.0)?); // r = 10.0
        assert!(!circ.transmits(7.0, 8.0)?); // r = sqrt(113) > 10

        let annular = Aperture::annular(3.0, 10.0);
        assert!(!annular.transmits(0.0, 0.0)?); // blocked by central obscuration
        assert!(!annular.transmits(2.0, 0.0)?);
        assert!(annular.transmits(5.0, 0.0)?); // in ring
        assert!(!annular.transmits(11.0, 0.0)?); // outside outer ring
        Ok(())
    }

    #[test]
    fn test_rectangular_and_polygonal_aperture() -> TestResult {
        let rect = Aperture {
            shape: ApertureShape::Rectangular {
                half_width_mm: 5.0,
                half_height_mm: 4.0,
            },
            decenter_x_mm: 1.0,
            decenter_y_mm: 0.0,
            is_obscuration: false,
        };

        assert!(rect.transmits(1.0, 0.0)?);
        assert!(rect.transmits(5.9, 3.9)?);
        assert!(!rect.transmits(6.5, 0.0)?);

        // Triangle polygon
        let tri = Aperture {
            shape: ApertureShape::Polygonal {
                vertices: vec![(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)],
            },
            decenter_x_mm: 0.0,
            decenter_y_mm: 0.0,
            is_obscuration: false,
        };
        assert!(tri.transmits(2.0, 2.0)?);
        assert!(!tri.transmits(8.0, 8.0)?);
        Ok(())
    }

    #[test]
    fn test_aperture_max_radial_extent_and_elliptical() -> TestResult {
        let circ = Aperture::circular(12.0);
        assert!((circ.max_radial_extent() - 12.0).abs() < 1e-12);

        let rect = Aperture::rectangular(3.0, 4.0);
        assert!((rect.max_radial_extent() - 5.0).abs() < 1e-12);

        let ellip = Aperture::elliptical(8.0, 6.0);
        assert!(ellip.transmits(8.0, 0.0)?);
        assert!(ellip.transmits(0.0, 6.0)?);
        assert!(!ellip.transmits(8.0, 0.1)?);
        assert!((ellip.max_radial_extent() - 8.0).abs() < 1e-12);

        let dec_rect = Aperture {
            shape: ApertureShape::Rectangular {
                half_width_mm: 3.0,
                half_height_mm: 4.0,
            },
            decenter_x_mm: 5.0,
            decenter_y_mm: 0.0,
            is_obscuration: false,
        };
        assert!((dec_rect.max_radial_extent() - 10.0).abs() < 1e-12);
        Ok(())
    }
}
