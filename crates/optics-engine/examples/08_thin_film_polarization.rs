//! Example 08: Optical Thin Films (TMM) & Polarization Ray Tracing
//!
//! Demonstrates physical wave optics and polarization state propagation:
//! - Transfer-Matrix Method (TMM) evaluation of a quarter-wave anti-reflection (AR) coating on N-BK7
//! - Evaluating reflectance spectra ($R_s, R_p, R_{\text{unpolarized}}$) across the visible spectrum (400–700 nm)
//! - Oblique incidence ($45^\circ$) polarization splitting and Brewster angle extinction
//! - Jones vector calculus: generation of circularly polarized light using a $45^\circ$ linear polarizer and quarter-wave plate
//! - Stokes vector representation: degree of polarization (DOP), linear (DOLP), and circular (DOCP)

use optics_core::polarization::{JonesMatrix, JonesVector};
use optics_core::thin_film::Coating;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OpticsLab Example 08: Optical Thin Films & Polarization ===\n");

    // 1. Single-layer Quarter-Wave Anti-Reflection (AR) Coating on N-BK7 Glass
    // Design wavelength: 550 nm (Green)
    // Ambient air (n0 = 1.0), Substrate N-BK7 (ns = 1.5168)
    // Optimal film index: n_film = sqrt(n0 * ns) = 1.2316
    let n_ambient = 1.0;
    let n_substrate = 1.5168;
    let lambda_design = 550.0;
    let ar_coating = Coating::quarter_wave_ar(n_ambient, n_substrate, lambda_design);

    println!("1. Single-Layer Anti-Reflection Coating on N-BK7 (λ0 = 550 nm):");
    println!("  Ambient Index:   n0 = {:.4}", n_ambient);
    println!("  Substrate Index: ns = {:.4}", n_substrate);
    println!(
        "  Optimal Film:    n1 = {:.4}, Thickness = {:.2} nm\n",
        (n_ambient * n_substrate).sqrt(),
        lambda_design / (4.0 * (n_ambient * n_substrate).sqrt())
    );

    // Evaluate normal incidence reflectance spectrum (0 degrees)
    println!("  Normal Incidence Reflectance Spectrum (θ = 0°):");
    println!("  Wavelength | Reflectance Rs | Reflectance Rp | Unpolarized R");
    println!("  -----------+----------------+----------------+--------------");
    let wavelengths = [400.0, 450.0, 500.0, 550.0, 600.0, 650.0, 700.0];
    for &wl in &wavelengths {
        let resp = ar_coating.evaluate(n_ambient, n_substrate, 0.0, wl)?;
        println!(
            "    {:5.0} nm | {:13.4}% | {:13.4}% | {:11.4}%",
            wl,
            resp.reflectance_s * 100.0,
            resp.reflectance_p * 100.0,
            resp.unpolarized_reflectance() * 100.0
        );
    }

    // 2. Oblique Incidence (45 degrees) and Polarization Splitting
    println!("\n2. Oblique Incidence Polarization Response (θ = 45°):");
    println!("  Wavelength | Reflectance Rs | Reflectance Rp | Polarization Ratio Rp/Rs");
    println!("  -----------+----------------+----------------+-------------------------");
    for &wl in &wavelengths {
        let resp_45 = ar_coating.evaluate(n_ambient, n_substrate, 45.0_f64.to_radians(), wl)?;
        let ratio = if resp_45.reflectance_s > 1e-8 {
            resp_45.reflectance_p / resp_45.reflectance_s
        } else {
            0.0
        };
        println!(
            "    {:5.0} nm | {:13.4}% | {:13.4}% | {:12.4}",
            wl,
            resp_45.reflectance_s * 100.0,
            resp_45.reflectance_p * 100.0,
            ratio
        );
    }

    // 3. Jones Vector Calculus & Circular Polarization Synthesis
    println!("\n3. Polarization State Synthesis (Jones Calculus):");

    // Input state: Unpolarized light passing through a linear polarizer at 45 degrees
    let pol_45 = JonesMatrix::linear_polarizer(45.0_f64.to_radians());
    let e_initial = JonesVector::linear_horizontal();
    let e_lin45 = pol_45.apply(e_initial).normalize();
    println!("  a) Initial horizontal state passed through 45° linear polarizer:");
    println!(
        "     Ex = {:.4} + {:.4}i, Ey = {:.4} + {:.4}i, Intensity = {:.4}",
        e_lin45.ex.re,
        e_lin45.ex.im,
        e_lin45.ey.re,
        e_lin45.ey.im,
        e_lin45.intensity()
    );

    // Pass linearly polarized 45 degree light through a Quarter-Wave Plate (fast axis at 0 degrees)
    let qwp_0 = JonesMatrix::quarter_wave_plate(0.0);
    let e_circ = qwp_0.apply(e_lin45);
    println!("  b) 45° state passed through Quarter-Wave Plate (fast axis at 0°):");
    println!(
        "     Ex = {:.4} + {:.4}i, Ey = {:.4} + {:.4}i, Intensity = {:.4}",
        e_circ.ex.re,
        e_circ.ex.im,
        e_circ.ey.re,
        e_circ.ey.im,
        e_circ.intensity()
    );

    // 4. Stokes Vector & Degree of Polarization
    println!("\n4. Stokes Vector & Polarimetric Metrics:");
    let stokes = e_circ.to_stokes();
    println!("  Stokes S0 (Total Intensity): {:.4}", stokes.s0);
    println!("  Stokes S1 (Horiz - Vert):    {:.4}", stokes.s1);
    println!("  Stokes S2 (+45° - -45°):     {:.4}", stokes.s2);
    println!("  Stokes S3 (Right - Left):    {:.4}", stokes.s3);
    println!(
        "  Degree of Polarization (DOP):         {:.4}",
        stokes.degree_of_polarization()
    );
    println!(
        "  Degree of Linear Polarization (DOLP): {:.4}",
        stokes.degree_of_linear_polarization()
    );
    println!(
        "  Degree of Circular Polarization (DOCP):{:.4}",
        stokes.degree_of_circular_polarization()
    );

    println!("\nThin film TMM and polarization simulation successfully completed.");
    Ok(())
}
