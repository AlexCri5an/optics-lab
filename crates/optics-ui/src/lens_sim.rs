//! Lens system simulation model and analytical computation state.
//!
//! Provides the data structures, preset loading, paraxial properties calculation,
//! 2D cross-section element polygon generation, spot diagram generation, and
//! transverse ray aberration fans for interactive display.

use optics_core::analysis::{
    PupilCoord, PupilPattern, RayFanData, SpotDiagram, SpotMetrics, launch_pupil_ray,
    trace_ray_fan, trace_spot_diagram,
};
use optics_core::conic::conic_sag;
use optics_core::prescription::{
    OpticalPrescription, ParaxialProperties, ResolvedPrescription, calculate_paraxial_properties,
    trace_sequential,
};
use optics_glass::catalog::GlassCatalogDb;
use optics_zemax::presets::{cooke_triplet_f4_5, double_gauss_50mm, plano_convex_100mm};

/// Available canonical lens system presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LensPreset {
    /// Cooke Triplet F/4.5 (f = 50 mm, 3-element).
    CookeTriplet,
    /// Double Gauss F/2.0 (f = 50 mm, 6-element).
    DoubleGauss,
    /// Plano-Convex N-BK7 singlet (f = 100 mm).
    PlanoConvex,
}

impl LensPreset {
    /// Returns the human-readable display name of the preset.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::CookeTriplet => "Cooke Triplet F/4.5",
            Self::DoubleGauss => "Double Gauss F/2.0",
            Self::PlanoConvex => "Plano-Convex Singlet (100 mm)",
        }
    }

    /// Loads the prescription for this preset.
    ///
    /// # Errors
    /// Returns an error string if parsing the embedded preset fails.
    pub fn load_prescription(&self) -> Result<OpticalPrescription, String> {
        match self {
            Self::CookeTriplet => cooke_triplet_f4_5().map_err(|e| format!("{e}")),
            Self::DoubleGauss => double_gauss_50mm().map_err(|e| format!("{e}")),
            Self::PlanoConvex => plano_convex_100mm().map_err(|e| format!("{e}")),
        }
    }
}

/// Active sub-tab in the lens analysis view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LensSubview {
    /// 2D optical layout showing glass cross-sections and ray fan paths.
    #[default]
    CrossSection,
    /// 2D geometric spot diagram scatter plot on image sensor.
    SpotDiagram,
    /// Transverse ray aberration fan curves (tangential & sagittal).
    RayFan,
    /// Numerical prescription surface editor table.
    PrescriptionTable,
}

/// A 2D polygonal lens element for cross-section rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct LensElement2D {
    /// Indices of front and rear surfaces (front_idx, rear_idx).
    pub surface_indices: (usize, usize),
    /// Material name (e.g. "N-SK16", "N-F2").
    pub material_name: String,
    /// Polygon boundary points in (z, y) coordinates (mm).
    pub boundary_points: Vec<[f64; 2]>,
}

/// A traced 2D meridional ray path through the lens system.
#[derive(Debug, Clone, PartialEq)]
pub struct TracedSequentialRay2D {
    /// Wavelength in nanometers.
    pub wavelength_nm: f64,
    /// Fractional pupil coordinate Py in [-1, 1].
    pub py: f64,
    /// Field angle in degrees.
    pub field_angle_deg: f64,
    /// Sequence of hit points along the ray in (z, y) coordinates (mm).
    pub vertices: Vec<[f64; 2]>,
    /// Whether the ray passed all apertures without vignetting.
    pub passed: bool,
}

/// Complete interactive state for lens system simulation and diagnostics.
pub struct LensSimulation {
    /// Current lens preset (if matching a known preset).
    pub preset: Option<LensPreset>,
    /// Editable optical prescription.
    pub prescription: OpticalPrescription,
    /// Glass catalog database for material resolution.
    catalog: GlassCatalogDb,
    /// Active diagnostic sub-view.
    pub active_subview: LensSubview,
    /// Selected field angle in degrees.
    pub field_angle_deg: f64,
    /// Primary evaluation wavelength in nanometers (default 587.56 nm d-line).
    pub wavelength_nm: f64,
    /// Number of hexapolar rings for spot diagram (default 5 -> 91 rays).
    pub spot_rings: usize,
    /// Number of pupil sampling points for ray fan (default 41).
    pub fan_points: usize,
    /// Cached paraxial first-order properties.
    pub paraxial: Option<ParaxialProperties>,
    /// Cached 2D lens element cross-sections.
    pub elements: Vec<LensElement2D>,
    /// Cached 2D traced meridional ray paths.
    pub cross_section_rays: Vec<TracedSequentialRay2D>,
    /// Cached geometric spot diagram data.
    pub spot_diagram: Option<SpotDiagram>,
    /// Cached spot diagram metrics.
    pub spot_metrics: Option<SpotMetrics>,
    /// Cached transverse ray aberration fans.
    pub ray_fan: Option<RayFanData>,
    /// Last error encountered during recalculation, if any.
    pub last_error: Option<String>,
}

impl LensSimulation {
    /// Creates a new `LensSimulation` initialized with the given preset.
    ///
    /// # Errors
    /// Returns an error string if glass catalog loading or prescription compilation fails.
    pub fn new(preset: LensPreset) -> Result<Self, String> {
        let catalog = GlassCatalogDb::load_bundled()
            .map_err(|e| format!("Failed to load glass catalog: {e}"))?;
        let prescription = preset
            .load_prescription()
            .map_err(|e| format!("Failed to load preset prescription: {e}"))?;

        let mut sim = Self {
            preset: Some(preset),
            prescription,
            catalog,
            active_subview: LensSubview::CrossSection,
            field_angle_deg: 0.0,
            wavelength_nm: 587.56,
            spot_rings: 5,
            fan_points: 41,
            paraxial: None,
            elements: Vec::new(),
            cross_section_rays: Vec::new(),
            spot_diagram: None,
            spot_metrics: None,
            ray_fan: None,
            last_error: None,
        };

        sim.recalculate();
        Ok(sim)
    }

    /// Switches to a different canonical lens preset and recalculates.
    pub fn select_preset(&mut self, preset: LensPreset) {
        match preset.load_prescription() {
            Ok(p) => {
                self.preset = Some(preset);
                self.prescription = p;
                self.recalculate();
            }
            Err(e) => {
                self.last_error = Some(format!("Failed to load preset: {e}"));
            }
        }
    }

    /// Loads an arbitrary [`OpticalPrescription`] (e.g. from an imported `.zmx` file) and recalculates.
    pub fn load_prescription_custom(&mut self, prescription: OpticalPrescription) {
        self.preset = None;
        self.prescription = prescription;
        self.recalculate();
    }

    /// Creates a new `LensSimulation` with a custom [`OpticalPrescription`].
    ///
    /// # Errors
    /// Returns an error string if glass catalog loading fails.
    pub fn from_prescription(prescription: OpticalPrescription) -> Result<Self, String> {
        let catalog = GlassCatalogDb::load_bundled()
            .map_err(|e| format!("Failed to load glass catalog: {e}"))?;

        let mut sim = Self {
            preset: None,
            prescription,
            catalog,
            active_subview: LensSubview::CrossSection,
            field_angle_deg: 0.0,
            wavelength_nm: 587.56,
            spot_rings: 5,
            fan_points: 41,
            paraxial: None,
            elements: Vec::new(),
            cross_section_rays: Vec::new(),
            spot_diagram: None,
            spot_metrics: None,
            ray_fan: None,
            last_error: None,
        };

        sim.recalculate();
        Ok(sim)
    }

    /// Recomputes all optical properties, cross-sections, spot diagram, and ray fans.
    pub fn recalculate(&mut self) {
        self.last_error = None;

        // Resolve prescription against glass catalog
        let resolved = match self.prescription.resolve(&self.catalog) {
            Ok(r) => r,
            Err(e) => {
                self.last_error = Some(format!("Material resolution error: {e}"));
                return;
            }
        };

        // 1. Compute Paraxial First-Order Properties
        match calculate_paraxial_properties(&resolved, self.wavelength_nm) {
            Ok(props) => self.paraxial = Some(props),
            Err(e) => {
                self.last_error = Some(format!("Paraxial calculation error: {e}"));
                self.paraxial = None;
            }
        }

        // 2. Generate 2D Lens Element Cross-Sections
        self.elements = build_lens_elements(&self.prescription);

        // 3. Trace 2D Cross-Section Rays
        self.cross_section_rays = trace_cross_section_rays(
            &resolved,
            self.paraxial.as_ref(),
            self.field_angle_deg,
            self.wavelength_nm,
        );

        // 4. Trace Spot Diagram & Evaluate Metrics
        match trace_spot_diagram(
            &resolved,
            (0.0, self.field_angle_deg),
            self.wavelength_nm,
            PupilPattern::Hexapolar {
                num_rings: self.spot_rings,
            },
        ) {
            Ok(spot) => {
                self.spot_metrics = Some(spot.metrics);
                self.spot_diagram = Some(spot);
            }
            Err(e) => {
                self.last_error = Some(format!("Spot diagram trace error: {e}"));
                self.spot_diagram = None;
                self.spot_metrics = None;
            }
        }

        // 5. Trace Transverse Ray Aberration Fans
        match trace_ray_fan(
            &resolved,
            (0.0, self.field_angle_deg),
            self.wavelength_nm,
            self.fan_points,
        ) {
            Ok(fan) => self.ray_fan = Some(fan),
            Err(e) => {
                self.last_error = Some(format!("Ray fan trace error: {e}"));
                self.ray_fan = None;
            }
        }
    }
}

/// Constructs 2D polygonal shapes for all physical glass elements in the system.
#[must_use]
pub fn build_lens_elements(prescription: &OpticalPrescription) -> Vec<LensElement2D> {
    let mut elements = Vec::new();
    let surfaces = &prescription.surfaces;
    let n = surfaces.len();
    if n < 2 {
        return elements;
    }

    // Cumulative z-vertex positions
    let mut z_vertices = Vec::with_capacity(n);
    let mut current_z = 0.0;
    for s in surfaces {
        z_vertices.push(current_z);
        current_z += s.thickness_mm;
    }

    let mut i = 0;
    while i + 1 < n {
        let front = &surfaces[i];
        let rear = &surfaces[i + 1];

        // An element exists when the medium between surface i and i+1 is not AIR
        let is_glass = !front.material_name.trim().eq_ignore_ascii_case("AIR")
            && !front.material_name.trim().is_empty();

        if is_glass {
            let z_front = z_vertices[i];
            let z_rear = z_vertices[i + 1];

            let c_front = if front.radius_mm.abs() < 1e-12 || front.radius_mm.is_infinite() {
                0.0
            } else {
                1.0 / front.radius_mm
            };
            let c_rear = if rear.radius_mm.abs() < 1e-12 || rear.radius_mm.is_infinite() {
                0.0
            } else {
                1.0 / rear.radius_mm
            };

            let h_front = front.semi_diameter_mm;
            let h_rear = rear.semi_diameter_mm;
            let h_max = h_front.max(h_rear);

            let mut pts = Vec::new();
            let steps = 24;

            // Front surface: top to bottom (y: +h_front -> -h_front)
            for step in 0..=steps {
                let frac = step as f64 / steps as f64;
                let y = h_front - 2.0 * h_front * frac;
                let sag = conic_sag(y, c_front, front.conic_constant).unwrap_or(0.0);
                pts.push([z_front + sag, y]);
            }

            // Lower rim to rear surface
            let rear_bottom_sag = conic_sag(-h_rear, c_rear, rear.conic_constant).unwrap_or(0.0);
            pts.push([z_rear + rear_bottom_sag, -h_max]);

            // Rear surface: bottom to top (y: -h_rear -> +h_rear)
            for step in 0..=steps {
                let frac = step as f64 / steps as f64;
                let y = -h_rear + 2.0 * h_rear * frac;
                let sag = conic_sag(y, c_rear, rear.conic_constant).unwrap_or(0.0);
                pts.push([z_rear + sag, y]);
            }

            // Upper rim back to front surface
            let front_top_sag = conic_sag(h_front, c_front, front.conic_constant).unwrap_or(0.0);
            pts.push([z_front + front_top_sag, h_max]);

            elements.push(LensElement2D {
                surface_indices: (i, i + 1),
                material_name: front.material_name.clone(),
                boundary_points: pts,
            });
        }
        i += 1;
    }

    elements
}

/// Traces a fan of meridional rays for 2D optical layout visualization.
#[must_use]
pub fn trace_cross_section_rays(
    prescription: &ResolvedPrescription,
    paraxial: Option<&ParaxialProperties>,
    field_angle_deg: f64,
    wavelength_nm: f64,
) -> Vec<TracedSequentialRay2D> {
    let mut rays = Vec::new();
    let num_rays = 7; // e.g. Py = -1.0, -0.66, -0.33, 0.0, 0.33, 0.66, 1.0
    let z_enp = paraxial.map_or(0.0, |p| p.entrance_pupil_pos_mm);

    for i in 0..num_rays {
        let py = -1.0 + 2.0 * (i as f64 / (num_rays - 1) as f64);
        let pupil = PupilCoord::new(0.0, py);
        let Ok(ray) = launch_pupil_ray(
            prescription,
            pupil,
            (0.0, field_angle_deg),
            wavelength_nm,
            z_enp,
        ) else {
            continue;
        };

        let (vertices, passed) = match trace_sequential(prescription, &ray) {
            Ok(record) => {
                let v = record.points.iter().map(|p| [p.z, p.y]).collect();
                (v, !record.is_vignetted)
            }
            Err(_) => (vec![[ray.origin.z, ray.origin.y]], false),
        };

        rays.push(TracedSequentialRay2D {
            wavelength_nm,
            py,
            field_angle_deg,
            vertices,
            passed,
        });
    }

    rays
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_cooke_triplet_preset_loading_and_recalculate() -> TestResult {
        let sim = LensSimulation::new(LensPreset::CookeTriplet)?;

        let paraxial = sim.paraxial.as_ref().ok_or("Paraxial missing")?;
        assert!(
            (paraxial.efl_mm - 50.0).abs() < 0.5,
            "Cooke Triplet EFL must be ~50 mm, got {}",
            paraxial.efl_mm
        );

        // Cooke Triplet has 3 physical glass elements
        assert_eq!(sim.elements.len(), 3);
        assert_eq!(sim.elements[0].material_name, "SK16");
        assert_eq!(sim.elements[1].material_name, "F2");
        assert_eq!(sim.elements[2].material_name, "SK16");

        // Cross-section rays traced
        assert_eq!(sim.cross_section_rays.len(), 7);

        // Spot diagram computed
        assert!(sim.spot_diagram.is_some());
        let metrics = sim.spot_metrics.as_ref().ok_or("Metrics missing")?;
        assert!(
            metrics.rms_radius_um > 0.0 && metrics.rms_radius_um < 50.0,
            "Cooke Triplet on-axis RMS blur expected between 0 and 50 um, got {}",
            metrics.rms_radius_um
        );

        // Ray fans computed
        let fan = sim.ray_fan.as_ref().ok_or("Fan missing")?;
        assert_eq!(fan.tangential.len(), 41);
        assert_eq!(fan.sagittal.len(), 41);

        Ok(())
    }

    #[test]
    fn test_double_gauss_preset_loading() -> TestResult {
        let sim = LensSimulation::new(LensPreset::DoubleGauss)?;

        let paraxial = sim.paraxial.as_ref().ok_or("Paraxial missing")?;
        assert!(
            (paraxial.efl_mm - 50.0).abs() < 1.0,
            "Double Gauss EFL must be ~50 mm, got {}",
            paraxial.efl_mm
        );
        assert!(
            (paraxial.f_number - 2.0).abs() < 0.2,
            "Double Gauss F-number must be ~2.0, got {}",
            paraxial.f_number
        );

        Ok(())
    }

    #[test]
    fn test_plano_convex_preset_loading() -> TestResult {
        let sim = LensSimulation::new(LensPreset::PlanoConvex)?;

        let paraxial = sim.paraxial.as_ref().ok_or("Paraxial missing")?;
        assert!(
            (paraxial.efl_mm - 100.0).abs() < 0.5,
            "Plano-Convex EFL must be ~100 mm, got {}",
            paraxial.efl_mm
        );
        assert_eq!(sim.elements.len(), 1);

        Ok(())
    }

    #[test]
    fn test_recalculate_performance_cooke_triplet() -> TestResult {
        let mut sim = LensSimulation::new(LensPreset::CookeTriplet)?;

        // Warm up
        sim.recalculate();

        let iterations = 5;
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            sim.recalculate();
        }
        let elapsed = start.elapsed();
        let ms_per_recalc = elapsed.as_secs_f64() * 1000.0 / (iterations as f64);

        // 60 FPS is 16.6 ms per frame; recalculation should be well under 25 ms in debug mode (<1 ms in release)
        assert!(
            ms_per_recalc < 25.0,
            "Recalculate took too long: {ms_per_recalc:.2} ms"
        );

        Ok(())
    }

    #[test]
    fn test_custom_prescription_loading() -> TestResult {
        let p = cooke_triplet_f4_5().map_err(|e| format!("{e}"))?;
        let mut sim = LensSimulation::from_prescription(p)?;

        assert!(sim.preset.is_none());
        assert_eq!(sim.elements.len(), 3);
        assert!(sim.paraxial.is_some());

        let dg = double_gauss_50mm().map_err(|e| format!("{e}"))?;
        sim.load_prescription_custom(dg);
        assert!(sim.preset.is_none());
        assert_eq!(sim.elements.len(), 6);
        assert!(sim.paraxial.is_some());

        Ok(())
    }
}
