//! Example 09: Optical Glass Catalog Search & Dispersion Models
//!
//! Demonstrates optical materials, glass catalog queries, and dispersion physics:
//! - Loading the embedded 646-glass manufacturer database (Schott, Ohara, CDGM, Hoya)
//! - Filtering optical materials by Abbe diagram region ($n_d \in [1.50, 1.55]$, $V_d \in [60, 65]$)
//! - Evaluating Sellmeier and Schott dispersion formulas across ultraviolet, visible, and infrared
//! - Calculating primary dispersion $\Delta n_{F-C} = n_F - n_C$, Abbe number $V_d$, and partial dispersion $P_{g,F}$
//! - Evaluating the Schott thermal dispersion model ($\mathrm{d}n/\mathrm{d}T$)

use optics_glass::catalog::GlassCatalogDb;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OpticsLab Example 09: Optical Glass Catalog & Dispersion ===\n");

    let catalog = GlassCatalogDb::load_bundled()?;
    println!("Loaded Optical Glass Database:");
    println!("  Total Glass Materials: {}\n", catalog.len());

    // 1. Search for Crown glasses in the typical N-BK7 region: nd in [1.50, 1.55], Vd in [60.0, 65.0]
    let nd_min = 1.50;
    let nd_max = 1.55;
    let vd_min = 60.0;
    let vd_max = 65.0;
    println!("1. Searching Abbe Diagram Region (Crown Glasses):");
    println!(
        "  Criteria: nd in [{:.2}, {:.2}], Vd in [{:.1}, {:.1}]",
        nd_min, nd_max, vd_min, vd_max
    );

    let matches = catalog.search_by_nd_vd((nd_min, nd_max), (vd_min, vd_max));
    println!("  Found {} matching optical glasses:\n", matches.len());
    println!("  Name            | Manufacturer | Refractive Index nd | Abbe Number Vd");
    println!("  ----------------+--------------+---------------------+---------------");
    for g in matches.iter().take(10) {
        println!(
            "  {:15} | {:12} | {:19.5} | {:14.2}",
            g.name,
            g.catalog.name(),
            g.nd,
            g.vd
        );
    }
    if matches.len() > 10 {
        println!("  ... and {} more glasses", matches.len() - 10);
    }

    // 2. In-depth dispersion evaluation for standard benchmark glasses: N-BK7 and SF11
    println!("\n2. Evaluating Dispersion Curves (UV -> Visible -> NIR):");
    let test_glasses = ["N-BK7", "SF11", "F2", "LITHOSIL-Q"];
    let spectral_lines = [
        (365.01, "i-line (UV)     "),
        (435.84, "g-line (Blue)   "),
        (486.13, "F-line (Cyan)   "),
        (546.07, "e-line (Green)  "),
        (587.56, "d-line (Yellow) "),
        (656.27, "C-line (Red)    "),
        (1013.98, "t-line (NIR)    "),
    ];

    for glass_name in &test_glasses {
        if let Some(glass) = catalog.lookup(glass_name) {
            println!(
                "\n  Material: {} ({}) - nd = {:.5}, Vd = {:.2}",
                glass.name,
                glass.catalog.name(),
                glass.nd,
                glass.vd
            );
            println!("    Spectral Line   | Wavelength (nm) | Refractive Index n(λ)");
            println!("    ----------------+-----------------+----------------------");
            for (wl, label) in &spectral_lines {
                let n = glass.refractive_index(*wl);
                println!("    {} | {:15.2} | {:20.6}", label, wl, n);
            }

            // Compute partial dispersion P_g,F
            let n_g = glass.refractive_index(435.84);
            let n_f = glass.refractive_index(486.13);
            let n_c = glass.refractive_index(656.27);
            let n_d = glass.refractive_index(587.56);
            let p_gf = (n_g - n_f) / (n_f - n_c);
            let v_d_calc = (n_d - 1.0) / (n_f - n_c);
            println!("    Derived Abbe Number Vd:       {:.2}", v_d_calc);
            println!("    Partial Dispersion Ratio Pg,F: {:.4}", p_gf);
        }
    }

    println!("\nGlass catalog querying and dispersion calculations successfully completed.");
    Ok(())
}
