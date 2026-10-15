//! Pure string formatting helpers for optics numbers, angles, and percentages.

use optics_core::BounceEvent;

/// Formats an angle given in radians to degrees with 2 decimal places.
#[must_use]
pub fn format_angle_deg(angle_rad: f64) -> String {
    format!("{:.2}°", angle_rad.to_degrees())
}

/// Formats a fractional value in $[0.0, 1.0]$ to a percentage with 1 decimal place.
#[must_use]
pub fn format_percent(fraction: f64) -> String {
    format!("{:.1}%", fraction * 100.0)
}

/// Formats a refractive index value to 6 decimal places.
#[must_use]
pub fn format_refractive_index(n: f64) -> String {
    format!("{:.6}", n)
}

/// Formats a wavelength given in nanometers with 1 decimal place.
#[must_use]
pub fn format_wavelength(wavelength_nm: f64) -> String {
    format!("{:.1} nm", wavelength_nm)
}

/// Returns a concise sentence-case descriptor for a ray interaction bounce event.
#[must_use]
pub fn format_bounce_event(event: BounceEvent) -> &'static str {
    match event {
        BounceEvent::Refract => "Refract",
        BounceEvent::Reflect => "Reflect",
        BounceEvent::TotalInternalReflection => "TIR",
        BounceEvent::Escape => "Escape",
        BounceEvent::Absorbed => "Absorbed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_formatting_precision() {
        assert_eq!(format_angle_deg(PI / 4.0), "45.00°");
        assert_eq!(format_angle_deg(PI / 3.0), "60.00°");
        assert_eq!(format_percent(0.8524), "85.2%");
        assert_eq!(format_percent(0.0003), "0.0%");
        assert_eq!(format_refractive_index(1.516800123), "1.516800");
        assert_eq!(format_wavelength(587.56), "587.6 nm");
        assert_eq!(
            format_bounce_event(BounceEvent::TotalInternalReflection),
            "TIR"
        );
    }
}
