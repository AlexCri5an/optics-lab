//! Canonical optical lens presets bundled as `.zmx` definitions.
//!
//! Provides the Cooke Triplet F/4.5, Double Gauss F/2.0, and Plano-Convex N-BK7 f = 100 mm.

use optics_core::OpticalPrescription;

use crate::error::FormatParseError;
use crate::model::FormatOpticalSystem;
use crate::parser::parse_zmx;

/// Canonical Cooke Triplet F/4.5 ($f = 50\,\text{mm}$) `.zmx` definition.
///
/// Designed by H. Dennis Taylor (1893). Minimal 3-element, 6-surface configuration
/// eliminating primary Seidel aberrations and chromatic aberration.
pub const COOKE_TRIPLET_F4_5_ZMX: &str = r#"NAME Cooke Triplet F/4.5
ENPD 10.000000
FNUM 4.500000
PWAV 2
WAVL 0.4861327 1.0000
WAVL 0.5875618 1.0000
WAVL 0.6562725 1.0000
YFLD 0.0000 14.0000 20.0000
SURF 0
  TYPE STANDARD
  CURV 0.0000000000
  DISZ INFINITY
SURF 1
  TYPE STANDARD
  CURV 0.0454264841
  DISZ 3.258960
  GLAS SK16
  DIAM 6.000000
SURF 2
  TYPE STANDARD
  CURV -0.0022948389
  DISZ 6.007550
  DIAM 6.000000
SURF 3
  TYPE STANDARD
  CURV -0.0450181153
  DISZ 0.999970
  GLAS F2
  DIAM 4.500000
SURF 4
  STOP
  TYPE STANDARD
  CURV 0.0492806992
  DISZ 4.750410
  DIAM 4.500000
SURF 5
  TYPE STANDARD
  CURV 0.0125496338
  DISZ 2.952080
  GLAS SK16
  DIAM 5.500000
SURF 6
  TYPE STANDARD
  CURV -0.0543616238
  DISZ 42.207780
  DIAM 5.500000
SURF 7
  TYPE STANDARD
  CURV 0.0000000000
  DISZ 0.000000
  DIAM 20.000000
"#;

/// Canonical Double Gauss F/2.0 ($f = 50\,\text{mm}$) `.zmx` definition.
///
/// Classic 6-element, 4-group configuration scaled to 50 mm EFL, forming
/// the basis of standard 35 mm SLR camera objectives.
pub const DOUBLE_GAUSS_F2_0_ZMX: &str = r#"NAME Double Gauss F/2.0
ENPD 25.000000
FNUM 2.000000
PWAV 2
WAVL 0.4861327 1.0000
WAVL 0.5875618 1.0000
WAVL 0.6562725 1.0000
YFLD 0.0000 10.0000 14.0000
SURF 0
  TYPE STANDARD
  CURV 0.0000000000
  DISZ INFINITY
SURF 1
  TYPE STANDARD
  CURV 0.0355856800
  DISZ 4.375000
  GLAS N-SSK2
  DIAM 8.000000
SURF 2
  TYPE STANDARD
  CURV 0.0131332000
  DISZ 0.250000
  DIAM 8.000000
SURF 3
  TYPE STANDARD
  CURV 0.0530748700
  DISZ 6.250000
  GLAS N-SK2
  DIAM 7.500000
SURF 4
  TYPE STANDARD
  CURV 0.0000000000
  DISZ 1.900000
  GLAS F5
  DIAM 7.500000
SURF 5
  TYPE STANDARD
  CURV 0.0825378800
  DISZ 8.184723
  DIAM 6.000000
SURF 6
  STOP
  TYPE STANDARD
  CURV 0.0000000000
  DISZ 6.873979
  DIAM 5.000000
SURF 7
  TYPE STANDARD
  CURV -0.0704788500
  DISZ 1.900000
  GLAS F5
  DIAM 5.000000
SURF 8
  TYPE STANDARD
  CURV 0.0000000000
  DISZ 5.500000
  GLAS N-SK16
  DIAM 7.000000
SURF 9
  TYPE STANDARD
  CURV -0.0527350200
  DISZ 0.250000
  DIAM 7.000000
SURF 10
  TYPE STANDARD
  CURV 0.0112732100
  DISZ 3.500000
  GLAS N-SK16
  DIAM 7.000000
SURF 11
  TYPE STANDARD
  CURV -0.0251850600
  DISZ 30.743768
  DIAM 7.000000
SURF 12
  TYPE STANDARD
  CURV 0.0000000000
  DISZ 0.000000
  DIAM 9.000000
"#;

/// Canonical Plano-Convex N-BK7 singlet ($f = 100\,\text{mm}$, $\text{EPD} = 20\,\text{mm}$) `.zmx` definition.
pub const PLANO_CONVEX_100MM_ZMX: &str = r#"NAME Plano-Convex N-BK7 f=100mm
ENPD 20.000000
FNUM 5.000000
PWAV 1
WAVL 0.5875618 1.0000
YFLD 0.0000
SURF 0
  TYPE STANDARD
  CURV 0.0000000000
  DISZ INFINITY
SURF 1
  TYPE STANDARD
  CURV 0.0193498450
  DISZ 5.000000
  GLAS N-BK7
  DIAM 12.000000
SURF 2
  TYPE STANDARD
  CURV 0.0000000000
  DISZ 96.703586
  DIAM 12.000000
SURF 3
  TYPE STANDARD
  CURV 0.0000000000
  DISZ 0.000000
  DIAM 12.000000
"#;

/// Loads the parsed [`FormatOpticalSystem`] for the Cooke Triplet F/4.5 preset.
///
/// # Errors
/// Returns [`FormatParseError`] if parsing fails.
pub fn cooke_triplet_system() -> Result<FormatOpticalSystem, FormatParseError> {
    parse_zmx(COOKE_TRIPLET_F4_5_ZMX)
}

/// Loads the [`OpticalPrescription`] for the Cooke Triplet F/4.5 preset.
///
/// # Errors
/// Returns [`FormatParseError`] if parsing fails.
pub fn cooke_triplet_f4_5() -> Result<OpticalPrescription, FormatParseError> {
    cooke_triplet_system().map(|s| s.to_optical_prescription())
}

/// Loads the parsed [`FormatOpticalSystem`] for the Double Gauss F/2.0 preset.
///
/// # Errors
/// Returns [`FormatParseError`] if parsing fails.
pub fn double_gauss_system() -> Result<FormatOpticalSystem, FormatParseError> {
    parse_zmx(DOUBLE_GAUSS_F2_0_ZMX)
}

/// Loads the [`OpticalPrescription`] for the Double Gauss F/2.0 preset.
///
/// # Errors
/// Returns [`FormatParseError`] if parsing fails.
pub fn double_gauss_50mm() -> Result<OpticalPrescription, FormatParseError> {
    double_gauss_system().map(|s| s.to_optical_prescription())
}

/// Loads the parsed [`FormatOpticalSystem`] for the Plano-Convex f = 100 mm preset.
///
/// # Errors
/// Returns [`FormatParseError`] if parsing fails.
pub fn plano_convex_system() -> Result<FormatOpticalSystem, FormatParseError> {
    parse_zmx(PLANO_CONVEX_100MM_ZMX)
}

/// Loads the [`OpticalPrescription`] for the Plano-Convex f = 100 mm preset.
///
/// # Errors
/// Returns [`FormatParseError`] if parsing fails.
pub fn plano_convex_100mm() -> Result<OpticalPrescription, FormatParseError> {
    plano_convex_system().map(|s| s.to_optical_prescription())
}

/// Canonical unoptimized Cooke Triplet benchmark for optical optimization testing.
///
/// Features rough starting curvatures where initial RMS spot size is large (~180 um),
/// providing a standard gold-standard benchmark to observe DLS aberration reduction.
pub const COOKE_TRIPLET_OPTIMIZATION_BENCHMARK_ZMX: &str = r#"NAME Cooke Triplet Optimization Benchmark
ENPD 10.000000
FNUM 5.000000
PWAV 2
WAVL 0.4861327 1.0000
WAVL 0.5875618 1.0000
WAVL 0.6562725 1.0000
YFLD 0.0000 7.0000 10.0000
SURF 0
  TYPE STANDARD
  CURV 0.0000000000
  DISZ INFINITY
SURF 1
  TYPE STANDARD
  CURV 0.0333333333
  DISZ 3.250000
  GLAS SK16
  DIAM 6.000000
SURF 2
  TYPE STANDARD
  CURV 0.0000000000
  DISZ 6.000000
  DIAM 6.000000
SURF 3
  TYPE STANDARD
  CURV -0.0333333333
  DISZ 1.000000
  GLAS F2
  DIAM 4.500000
SURF 4
  STOP
  TYPE STANDARD
  CURV 0.0333333333
  DISZ 4.750000
  DIAM 4.500000
SURF 5
  TYPE STANDARD
  CURV 0.0250000000
  DISZ 2.950000
  GLAS SK16
  DIAM 5.500000
SURF 6
  TYPE STANDARD
  CURV -0.0333333333
  DISZ 43.500000
  DIAM 5.500000
SURF 7
  TYPE STANDARD
  CURV 0.0000000000
  DISZ 0.000000
  DIAM 10.000000
"#;

/// Loads the parsed [`FormatOpticalSystem`] for the Cooke Triplet optimization benchmark.
///
/// # Errors
/// Returns [`FormatParseError`] if parsing fails.
pub fn cooke_triplet_benchmark_system() -> Result<FormatOpticalSystem, FormatParseError> {
    parse_zmx(COOKE_TRIPLET_OPTIMIZATION_BENCHMARK_ZMX)
}

/// Loads the [`OpticalPrescription`] for the Cooke Triplet optimization benchmark.
///
/// # Errors
/// Returns [`FormatParseError`] if parsing fails.
pub fn cooke_triplet_benchmark() -> Result<OpticalPrescription, FormatParseError> {
    cooke_triplet_benchmark_system().map(|s| s.to_optical_prescription())
}
