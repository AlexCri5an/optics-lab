//! Data models representing optical systems and surfaces.

use std::fmt;
use std::str::FromStr;

use optics_core::{Aperture, OpticalPrescription, OpticalSurface};
use serde::{Deserialize, Serialize};

/// Supported surface profile types in optical systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SurfaceType {
    /// Standard spherical or conic revolution surface.
    #[default]
    Standard,
    /// Even polynomial asphere ($z(r) = \frac{c r^2}{1 + \sqrt{1 - (1+k)c^2 r^2}} + \sum \alpha_i r^{2i}$).
    EvenAsphere,
    /// Toroidal surface of revolution.
    Toroidal,
    /// Biconic surface with independent x and y curvatures.
    Biconic,
    /// Zernike polynomial sag deformation.
    ZernikeSag,
    /// Forbes Q-con orthogonal aspheric surface.
    ForbesQcon,
    /// Forbes Q-bfs orthogonal aspheric surface.
    ForbesQbfs,
    /// Coordinate break / 3D Euler tilt and decenter.
    CoordinateBreak,
    /// Diffractive phase surface.
    Diffractive,
    /// Radial gradient (Selfoc/Wood lens, GRADIENT_1 / GRIN1).
    Gradient1,
    /// Axial gradient (GRADIENT_2 / GRIN2).
    Gradient2,
    /// Luneburg / spherical gradient (GRADIENT_3 / GRIN3).
    Gradient3,
}

impl fmt::Display for SurfaceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Standard => write!(f, "STANDARD"),
            Self::EvenAsphere => write!(f, "EVENASPH"),
            Self::Toroidal => write!(f, "TOROIDAL"),
            Self::Biconic => write!(f, "BICONIC"),
            Self::ZernikeSag => write!(f, "ZERNIKE"),
            Self::ForbesQcon => write!(f, "QCON"),
            Self::ForbesQbfs => write!(f, "QBFS"),
            Self::CoordinateBreak => write!(f, "COORDBRK"),
            Self::Diffractive => write!(f, "DIFFRACT"),
            Self::Gradient1 => write!(f, "GRADIENT_1"),
            Self::Gradient2 => write!(f, "GRADIENT_2"),
            Self::Gradient3 => write!(f, "GRADIENT_3"),
        }
    }
}

impl FromStr for SurfaceType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim().to_uppercase();
        Ok(match trimmed.as_str() {
            "EVENASPH" | "EVEN_ASPHERE" | "ASPH" => Self::EvenAsphere,
            "TOROIDAL" => Self::Toroidal,
            "BICONIC" => Self::Biconic,
            "ZERNIKE" | "ZERNIKESAG" => Self::ZernikeSag,
            "QCON" | "QCONIC" | "FORBES_QCON" => Self::ForbesQcon,
            "QBFS" | "Q2D" | "FORBES_QBFS" => Self::ForbesQbfs,
            "COORDBRK" | "COORDINATE_BREAK" => Self::CoordinateBreak,
            "DIFFRACT" | "DIFFRACTIVE" | "BINARY2" => Self::Diffractive,
            "GRADIENT_1" | "GRIN1" | "SELFOC" => Self::Gradient1,
            "GRADIENT_2" | "GRIN2" | "AXIAL_GRIN" => Self::Gradient2,
            "GRADIENT_3" | "GRIN3" | "LUNEBURG" => Self::Gradient3,
            _ => Self::Standard,
        })
    }
}

/// A single surface in an optical system document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormatSurface {
    /// 0-based surface index (0 = Object surface).
    pub index: usize,
    /// Surface comment / note (`COMM`).
    pub comment: Option<String>,
    /// Geometric surface type profile.
    pub surface_type: SurfaceType,
    /// Radius of curvature in millimeters ($R = 1/c$). Planar surfaces have `f64::INFINITY`.
    #[serde(with = "optics_core::prescription::f64_infinity_serde")]
    pub radius_mm: f64,
    /// Surface curvature $c = 1 / R$ in $\text{mm}^{-1}$.
    pub curvature: f64,
    /// Axial vertex separation to the next surface in millimeters.
    #[serde(with = "optics_core::prescription::f64_infinity_serde")]
    pub thickness_mm: f64,
    /// Glass material designation (e.g. "N-BK7", "SK16", "F2", "AIR", or empty).
    pub material_name: String,
    /// Clear semi-diameter / radial aperture in millimeters.
    pub semi_diameter_mm: f64,
    /// Conic constant $k$ (0 for spherical, -1 for parabolic).
    pub conic_constant: f64,
    /// Whether this surface is flagged as the Aperture Stop.
    pub is_stop: bool,
    /// Extra numeric surface parameters (`PARM 1`, `PARM 2`, ...).
    pub parameters: Vec<f64>,
    /// Optional physical aperture shape and obscuration boundary (`CLAP` / `OBST` / `APER`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aperture: Option<Aperture>,
    /// Optional thin-film optical coating (`COAT` operand).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coating: Option<optics_core::SurfaceCoating>,
    /// Preserved unrecognized surface-level operand lines for lossless round-tripping.
    pub extra_lines: Vec<String>,
}

impl Default for FormatSurface {
    fn default() -> Self {
        Self {
            index: 0,
            comment: None,
            surface_type: SurfaceType::Standard,
            radius_mm: f64::INFINITY,
            curvature: 0.0,
            thickness_mm: 0.0,
            material_name: String::new(),
            semi_diameter_mm: 10.0,
            conic_constant: 0.0,
            is_stop: false,
            parameters: Vec::new(),
            aperture: None,
            coating: None,
            extra_lines: Vec::new(),
        }
    }
}

/// A parsed optical system document representation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormatOpticalSystem {
    /// Optical design name / title from `NAME` operand.
    pub name: String,
    /// Entrance pupil diameter in millimeters (`ENPD`).
    pub entrance_pupil_diameter_mm: f64,
    /// Working image-space F-number (`FNUM`).
    pub f_number: f64,
    /// Field type (`FTYP`, e.g. "ANGLE", "OBJ_HEIGHT", "PARAXIAL_IMAGE_HEIGHT").
    pub field_type: String,
    /// Sampled wavelengths in micrometers ($\mu\text{m}$) (`WAVL` / `WAVM`).
    pub wavelengths_um: Vec<f64>,
    /// Relative wavelength weightings.
    pub wavelength_weights: Vec<f64>,
    /// 0-based index of primary wavelength reference (`PWAV` - 1).
    pub primary_wavelength_index: usize,
    /// Field angle coordinates in degrees (`YFLD` / `YFLN`).
    pub field_angles_deg: Vec<f64>,
    /// Referenced glass catalogs (`GCAT`).
    pub glass_catalogs: Vec<String>,
    /// Sequential surfaces from Object (`SURF 0`) to Image plane (`SURF N`).
    pub surfaces: Vec<FormatSurface>,
    /// Preserved unrecognized header/system-level operand lines for lossless round-tripping.
    pub extra_lines: Vec<String>,
}

impl Default for FormatOpticalSystem {
    fn default() -> Self {
        Self {
            name: "Untitled System".to_string(),
            entrance_pupil_diameter_mm: 10.0,
            f_number: 4.5,
            field_type: "ANGLE".to_string(),
            wavelengths_um: vec![0.5875618], // Helium d-line 587.56 nm
            wavelength_weights: vec![1.0],
            primary_wavelength_index: 0,
            field_angles_deg: vec![0.0],
            glass_catalogs: vec!["SCHOTT".to_string()],
            surfaces: Vec::new(),
            extra_lines: Vec::new(),
        }
    }
}

impl FormatOpticalSystem {
    /// Converts this parsed optical system into an `optics-core` [`OpticalPrescription`].
    ///
    /// # Conversion Details
    /// - Wavelengths are converted from micrometers ($\mu\text{m}$) to nanometers ($\text{nm}$) ($\times 1000$).
    /// - If the system contains an Object surface (`SURF 0`), optical surfaces starting at
    ///   `SURF 1` are extracted for the sequential prescription.
    #[must_use]
    pub fn to_optical_prescription(&self) -> OpticalPrescription {
        let mut wavelengths_nm: Vec<f64> = Vec::new();
        for &w in &self.wavelengths_um {
            let nm = w * 1000.0;
            if !wavelengths_nm.iter().any(|&existing| (existing - nm).abs() < 1e-2) {
                wavelengths_nm.push(nm);
            }
        }

        // If surfaces contain SURF 0 and optical surfaces > 0, filter out object surface
        let has_optical_surfaces = self.surfaces.iter().any(|s| s.index > 0);
        let optical_surfs: Vec<&FormatSurface> = if has_optical_surfaces {
            self.surfaces.iter().filter(|s| s.index > 0).collect()
        } else {
            self.surfaces.iter().collect()
        };

        let surfaces: Vec<OpticalSurface> = optical_surfs
            .into_iter()
            .map(|s| OpticalSurface {
                surface_index: s.index,
                radius_mm: s.radius_mm,
                conic_constant: s.conic_constant,
                thickness_mm: s.thickness_mm,
                semi_diameter_mm: s.semi_diameter_mm,
                material_name: if s.material_name.is_empty() {
                    "AIR".to_string()
                } else {
                    s.material_name.clone()
                },
                is_stop: s.is_stop,
                asphere_coefficients: if s.surface_type == SurfaceType::EvenAsphere {
                    s.parameters.clone()
                } else {
                    Vec::new()
                },
                coordinate_break: if s.surface_type == SurfaceType::CoordinateBreak {
                    let dx = s.parameters.first().copied().unwrap_or(0.0);
                    let dy = s.parameters.get(1).copied().unwrap_or(0.0);
                    let tx = s.parameters.get(2).copied().unwrap_or(0.0);
                    let ty = s.parameters.get(3).copied().unwrap_or(0.0);
                    let tz = s.parameters.get(4).copied().unwrap_or(0.0);
                    let order = s.parameters.get(5).copied().unwrap_or(0.0);
                    Some(optics_core::CoordinateBreak {
                        decenter_x_mm: dx,
                        decenter_y_mm: dy,
                        decenter_z_mm: 0.0,
                        tilt_x_deg: tx,
                        tilt_y_deg: ty,
                        tilt_z_deg: tz,
                        tilt_first: order != 0.0,
                    })
                } else {
                    None
                },
                grin_profile: match s.surface_type {
                    SurfaceType::Gradient1 => {
                        let n0 = s.parameters.first().copied().unwrap_or(1.6);
                        let a = s.parameters.get(1).copied().unwrap_or(0.01);
                        Some(optics_core::GrinProfile::Radial { n0, a })
                    }
                    SurfaceType::Gradient2 => {
                        let n0 = s.parameters.first().copied().unwrap_or(1.5);
                        let n1 = s.parameters.get(1).copied().unwrap_or(0.0);
                        let n2 = s.parameters.get(2).copied().unwrap_or(0.0);
                        Some(optics_core::GrinProfile::Axial { n0, n1, n2 })
                    }
                    SurfaceType::Gradient3 => {
                        let n0 = s.parameters.first().copied().unwrap_or(1.0);
                        let r = s.parameters.get(1).copied().unwrap_or(s.semi_diameter_mm);
                        Some(optics_core::GrinProfile::Luneburg { n0, radius_mm: r })
                    }
                    _ => None,
                },
                profile: match s.surface_type {
                    SurfaceType::Biconic => {
                        let rx = s.radius_mm;
                        let kx = s.conic_constant;
                        let ry = s.parameters.first().copied().unwrap_or(rx);
                        let ky = s.parameters.get(1).copied().unwrap_or(kx);
                        optics_core::BiconicSurface::from_radii(rx, kx, ry, ky, s.semi_diameter_mm)
                            .ok()
                            .map(optics_core::SurfaceProfile::Biconic)
                    }
                    SurfaceType::Toroidal => {
                        let rx = s.radius_mm;
                        let ry = s.parameters.first().copied().unwrap_or(rx);
                        let ky = s.parameters.get(1).copied().unwrap_or(s.conic_constant);
                        optics_core::ToroidalSurface::from_radii(rx, ry, ky, s.semi_diameter_mm)
                            .ok()
                            .map(optics_core::SurfaceProfile::Toroidal)
                    }
                    SurfaceType::ForbesQcon => {
                        let c = if s.radius_mm.is_infinite() || s.radius_mm.abs() < 1e-15 {
                            0.0
                        } else {
                            1.0 / s.radius_mm
                        };
                        let r_max = s
                            .parameters
                            .first()
                            .copied()
                            .filter(|&r| r > 0.0)
                            .unwrap_or(s.semi_diameter_mm);
                        let coeffs: Vec<f64> = s.parameters.iter().skip(1).copied().collect();
                        optics_core::ForbesQconSurface::new(c, s.conic_constant, r_max, coeffs)
                            .ok()
                            .map(optics_core::SurfaceProfile::ForbesQcon)
                    }
                    SurfaceType::ForbesQbfs => {
                        let c = if s.radius_mm.is_infinite() || s.radius_mm.abs() < 1e-15 {
                            0.0
                        } else {
                            1.0 / s.radius_mm
                        };
                        let r_max = s
                            .parameters
                            .first()
                            .copied()
                            .filter(|&r| r > 0.0)
                            .unwrap_or(s.semi_diameter_mm);
                        let coeffs: Vec<f64> = s.parameters.iter().skip(1).copied().collect();
                        optics_core::ForbesQbfsSurface::new(c, r_max, coeffs)
                            .ok()
                            .map(optics_core::SurfaceProfile::ForbesQbfs)
                    }
                    SurfaceType::ZernikeSag => {
                        let c = if s.radius_mm.is_infinite() || s.radius_mm.abs() < 1e-15 {
                            0.0
                        } else {
                            1.0 / s.radius_mm
                        };
                        let norm_r = s
                            .parameters
                            .first()
                            .copied()
                            .filter(|&r| r > 0.0)
                            .unwrap_or(s.semi_diameter_mm);
                        let coeffs: Vec<(usize, f64)> = s
                            .parameters
                            .iter()
                            .skip(1)
                            .enumerate()
                            .map(|(i, &v)| (i + 1, v))
                            .collect();
                        optics_core::ZernikeSagSurface::new(c, s.conic_constant, norm_r, coeffs, s.semi_diameter_mm)
                            .ok()
                            .map(optics_core::SurfaceProfile::Zernike)
                    }
                    _ => None,
                },
                aperture: s.aperture.clone(),
                coating: s.coating.clone(),
            })
            .collect();

        OpticalPrescription {
            name: self.name.clone(),
            entrance_pupil_diameter_mm: self.entrance_pupil_diameter_mm,
            field_angles_deg: if self.field_angles_deg.is_empty() {
                vec![0.0]
            } else {
                self.field_angles_deg.clone()
            },
            wavelengths_nm: if wavelengths_nm.is_empty() {
                vec![587.56]
            } else {
                wavelengths_nm
            },
            primary_wavelength_index: self
                .primary_wavelength_index
                .min(self.wavelengths_um.len().saturating_sub(1)),
            surfaces,
        }
    }

    /// Creates a [`FormatOpticalSystem`] from an `optics-core` [`OpticalPrescription`].
    ///
    /// Automatically prepends an Object surface (`SURF 0`) with infinite thickness
    /// and converts wavelengths from nanometers ($\text{nm}$) to micrometers ($\mu\text{m}$).
    #[must_use]
    pub fn from_optical_prescription(prescription: &OpticalPrescription) -> Self {
        let wavelengths_um: Vec<f64> = prescription.wavelengths_nm.iter().map(|&w| w / 1000.0).collect();

        let wavelength_weights = vec![1.0; wavelengths_um.len()];

        let mut surfaces = Vec::with_capacity(prescription.surfaces.len() + 1);

        // Prepended Object surface at infinity
        surfaces.push(FormatSurface {
            index: 0,
            comment: Some("Object".to_string()),
            surface_type: SurfaceType::Standard,
            radius_mm: f64::INFINITY,
            curvature: 0.0,
            thickness_mm: f64::INFINITY,
            material_name: "AIR".to_string(),
            semi_diameter_mm: prescription.entrance_pupil_diameter_mm * 0.5,
            conic_constant: 0.0,
            is_stop: false,
            parameters: Vec::new(),
            aperture: None,
            coating: None,
            extra_lines: Vec::new(),
        });

        for surf in &prescription.surfaces {
            let curv = if surf.radius_mm.is_infinite() || surf.radius_mm.abs() < 1e-12 {
                0.0
            } else {
                1.0 / surf.radius_mm
            };

            let (surface_type, parameters) = if let Some(cb) = &surf.coordinate_break {
                (
                    SurfaceType::CoordinateBreak,
                    vec![
                        cb.decenter_x_mm,
                        cb.decenter_y_mm,
                        cb.tilt_x_deg,
                        cb.tilt_y_deg,
                        cb.tilt_z_deg,
                        if cb.tilt_first { 1.0 } else { 0.0 },
                    ],
                )
            } else if let Some(grin) = &surf.grin_profile {
                match grin {
                    optics_core::GrinProfile::Radial { n0, a } => (SurfaceType::Gradient1, vec![*n0, *a]),
                    optics_core::GrinProfile::Axial { n0, n1, n2 } => (SurfaceType::Gradient2, vec![*n0, *n1, *n2]),
                    optics_core::GrinProfile::Luneburg { n0, radius_mm } => {
                        (SurfaceType::Gradient3, vec![*n0, *radius_mm])
                    }
                }
            } else if let Some(prof) = &surf.profile {
                match prof {
                    optics_core::SurfaceProfile::Biconic(b) => (SurfaceType::Biconic, vec![b.ry_mm(), b.ky]),
                    optics_core::SurfaceProfile::Toroidal(t) => (SurfaceType::Toroidal, vec![t.ry_mm(), t.ky]),
                    optics_core::SurfaceProfile::ForbesQcon(q) => {
                        let mut params = vec![q.r_max_mm];
                        params.extend(&q.coefficients);
                        (SurfaceType::ForbesQcon, params)
                    }
                    optics_core::SurfaceProfile::ForbesQbfs(q) => {
                        let mut params = vec![q.r_max_mm];
                        params.extend(&q.coefficients);
                        (SurfaceType::ForbesQbfs, params)
                    }
                    optics_core::SurfaceProfile::Zernike(z) => {
                        let mut params = vec![z.norm_radius_mm];
                        for &(_, val) in &z.coefficients {
                            params.push(val);
                        }
                        (SurfaceType::ZernikeSag, params)
                    }
                    _ => (SurfaceType::Standard, Vec::new()),
                }
            } else if !surf.asphere_coefficients.is_empty() {
                (SurfaceType::EvenAsphere, surf.asphere_coefficients.clone())
            } else {
                (SurfaceType::Standard, Vec::new())
            };

            surfaces.push(FormatSurface {
                index: surf.surface_index,
                comment: None,
                surface_type,
                radius_mm: surf.radius_mm,
                curvature: curv,
                thickness_mm: surf.thickness_mm,
                material_name: surf.material_name.clone(),
                semi_diameter_mm: surf.semi_diameter_mm,
                conic_constant: surf.conic_constant,
                is_stop: surf.is_stop,
                parameters,
                aperture: surf.aperture.clone(),
                coating: surf.coating.clone(),
                extra_lines: Vec::new(),
            });
        }

        Self {
            name: prescription.name.clone(),
            entrance_pupil_diameter_mm: prescription.entrance_pupil_diameter_mm,
            f_number: 4.5,
            field_type: "ANGLE".to_string(),
            wavelengths_um,
            wavelength_weights,
            primary_wavelength_index: prescription.primary_wavelength_index,
            field_angles_deg: prescription.field_angles_deg.clone(),
            glass_catalogs: vec!["SCHOTT".to_string()],
            surfaces,
            extra_lines: Vec::new(),
        }
    }
}
