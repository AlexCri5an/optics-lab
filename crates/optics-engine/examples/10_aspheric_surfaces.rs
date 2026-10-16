//! Example 10: Advanced Aspheric Surfaces (Even Asphere, Forbes Q-Type, Zernike Sag)
//!
//! Demonstrates advanced geometric optical surface modeling:
//! - Standard Quadric / Conic surfaces (Sphere $k = 0$, Paraboloid $k = -1$, Hyperboloid $k < -1$)
//! - Even Asphere polynomial expansion ($r^4, r^6, r^8$)
//! - Forbes $Q_{\text{con}}$ orthogonal aspheric polynomials via 3-term recurrence
//! - Zernike circular pupil sag expansion (Noll indexing)
//! - Rigorous evaluation of exact analytical sag $z(r)$ and surface normal unit length $\|\hat{\mathbf{n}}\| = 1.000000000000$

use optics_core::conic::ConicSurface;
use optics_core::surface::SurfaceProfile;
use optics_core::surface::asphere::EvenAsphere;
use optics_core::surface::forbes::ForbesQconSurface;
use optics_core::surface::polynomial::ZernikeSagSurface;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OpticsLab Example 10: Advanced Aspheric & Freeform Surfaces ===\n");

    let vertex_radius = 50.0;
    let curvature = 1.0 / vertex_radius; // c = 0.02 mm^-1
    let aperture_radius = 15.0; // r_max = 15 mm

    // 1. Conic Family (Sphere, Paraboloid, Hyperboloid)
    let sphere = SurfaceProfile::Conic(ConicSurface {
        curvature,
        conic_constant: 0.0,
        vertex_z: 0.0,
        semi_diam_mm: aperture_radius,
    });
    let parabola = SurfaceProfile::Conic(ConicSurface {
        curvature,
        conic_constant: -1.0,
        vertex_z: 0.0,
        semi_diam_mm: aperture_radius,
    });
    let hyperbola = SurfaceProfile::Conic(ConicSurface {
        curvature,
        conic_constant: -2.5,
        vertex_z: 0.0,
        semi_diam_mm: aperture_radius,
    });

    // 2. Even Asphere with 4th and 6th order deformation terms
    // z(r) = c r^2 / (1 + sqrt(1 - (1+k) c^2 r^2)) + a4 r^4 + a6 r^6
    let even_asph = SurfaceProfile::EvenAsphere(EvenAsphere::new(
        curvature,
        0.0,                     // base sphere
        vec![0.0, 1e-6, -2e-10], // a2=0, a4=1e-6, a6=-2e-10
        aperture_radius,
    )?);

    // 3. Forbes Q_con Orthogonal Asphere
    let forbes_qcon = SurfaceProfile::ForbesQcon(ForbesQconSurface::new(
        curvature,
        0.0, // base sphere
        aperture_radius,
        vec![0.015, -0.005, 0.001], // orthogonal Forbes coefficients
    )?);

    // 4. Zernike Sag Surface (Noll index 11: Primary Spherical Aberration Z(4, 0))
    let zernike_sag = SurfaceProfile::Zernike(ZernikeSagSurface::new(
        curvature,
        0.0,
        aperture_radius,
        vec![(11, 0.005)], // 5 µm of primary spherical aberration
        aperture_radius,
    )?);

    // 5. Evaluate and Compare Sags and Normal Vectors across Aperture Radius
    println!("Evaluating Surface Profiles across Radial Aperture (r from 0 to 15 mm):");
    println!(
        "  Radius (mm) | Sphere Sag | Parabola Sag | Hyperbola Sag | Even Asph Sag | Forbes Qcon | Zernike Sag"
    );
    println!(
        "  ------------+------------+--------------+---------------+---------------+-------------+------------"
    );

    let radii = [0.0, 3.0, 6.0, 9.0, 12.0, 15.0];
    for &r in &radii {
        let sag_sph = sphere.sag(r, 0.0)?;
        let sag_par = parabola.sag(r, 0.0)?;
        let sag_hyp = hyperbola.sag(r, 0.0)?;
        let sag_eas = even_asph.sag(r, 0.0)?;
        let sag_qcn = forbes_qcon.sag(r, 0.0)?;
        let sag_zen = zernike_sag.sag(r, 0.0)?;

        println!(
            "    {:5.1} mm | {:8.4} mm | {:10.4} mm | {:11.4} mm | {:11.4} mm | {:9.4} mm | {:9.4} mm",
            r, sag_sph, sag_par, sag_hyp, sag_eas, sag_qcn, sag_zen
        );
    }

    // 6. Verify Surface Normal Unit-Length Invariant: ||n(r)|| = 1.000000000000
    println!("\nSurface Normal Unit-Length Verification (at r = 12.0 mm):");
    let test_r = 12.0;

    let surfaces = [
        ("Base Sphere", &sphere),
        ("Paraboloid", &parabola),
        ("Hyperboloid", &hyperbola),
        ("Even Asphere", &even_asph),
        ("Forbes Q_con", &forbes_qcon),
        ("Zernike Sag", &zernike_sag),
    ];

    println!("  Surface Type   | Normal Vector n = (nx, ny, nz)       | Normal Length ||n||");
    println!("  ---------------+--------------------------------------+--------------------");
    for (name, surf) in &surfaces {
        let norm = surf.normal(test_r, 0.0)?;
        let len = (norm.x * norm.x + norm.y * norm.y + norm.z * norm.z).sqrt();
        println!(
            "  {:14} | ({:+8.5}, {:+8.5}, {:+8.5}) | {:.12}",
            name, norm.x, norm.y, norm.z, len
        );
    }

    println!("\nAspheric and freeform surface evaluation successfully completed.");
    Ok(())
}
