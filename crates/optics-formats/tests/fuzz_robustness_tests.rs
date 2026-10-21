//! Fuzzing and robustness stress-testing for OpticsLab optical file parsers.
//!
//! Validates that `.zmx`, `.seq` (CODE V), `.len` (OSLO), and project JSON
//! parsers never panic or crash under corrupted, truncated, malformed, non-ASCII,
//! or adversarial inputs.

use optics_formats::presets::{COOKE_TRIPLET_F4_5_ZMX, cooke_triplet_f4_5};
use optics_formats::{
    CodeVSystem, OpticsProjectWorkspace, OsloSystem, parse_len, parse_seq, parse_zmx, parse_zmx_bytes, write_len,
    write_seq,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Deterministic 64-bit Xorshift PRNG for fuzzing mutations.
struct FuzzRng {
    state: u64,
}

impl FuzzRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x4d595df4d0f33173 } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    fn next_usize(&mut self, max: usize) -> usize {
        if max == 0 {
            0
        } else {
            (self.next_u64() % (max as u64)) as usize
        }
    }

    fn next_u8(&mut self) -> u8 {
        (self.next_u64() & 0xFF) as u8
    }
}

/// Stress-tests parsers with empty, single-byte, and null-filled inputs.
#[test]
fn test_fuzz_boundary_and_null_bytes() -> TestResult {
    let inputs: [&[u8]; 8] = [
        b"",
        b"\0",
        b"\0\0\0\0\0\0\0\0",
        b"\n",
        b"\r\n\r\n",
        b" ",
        b"\t\t\t\t",
        b"\xFF\xFE\x00\x00",
    ];

    for (i, bytes) in inputs.iter().enumerate() {
        // ZMX
        let _ = parse_zmx_bytes(bytes);
        if let Ok(s) = std::str::from_utf8(bytes) {
            let _ = parse_zmx(s);
            let _ = parse_seq(s);
            let _ = parse_len(s);
            let _ = OpticsProjectWorkspace::from_json(s);
        }
        println!("[FUZZ] Boundary input {i} handled safely without panic");
    }
    Ok(())
}

/// Stress-tests parsers with extreme floating-point representations and NaN/Inf tokens.
#[test]
fn test_fuzz_extreme_floats_and_malformed_tokens() -> TestResult {
    let extreme_float_tokens = [
        "NaN",
        "+NaN",
        "-NaN",
        "Infinity",
        "-Infinity",
        "+Infinity",
        "inf",
        "-inf",
        "1e308",
        "-1e308",
        "1e-308",
        "1e99999",
        "-1e99999",
        "1e-99999",
        "0.00000000000000000000000000000000000000000000000000001",
        "999999999999999999999999999999999999999999999999999999999999",
        "null",
        "undefined",
        "true",
        "false",
        "\"string_instead_of_number\"",
        "1.2.3.4",
        "---",
        "+++",
        "1e+e+1",
    ];

    for tok in extreme_float_tokens {
        // Construct malformed ZMX snippet
        let zmx_snippet = format!(
            "VERS 0\nSURF 1\n  CURV {}\n  DISZ {}\n  DIAM {}\n  CONI {}\n",
            tok, tok, tok, tok
        );
        let _ = parse_zmx(&zmx_snippet);

        // Construct malformed SEQ snippet
        let seq_snippet = format!("TIT 'Test'\nSO 0\nS 1\nRDY {}\nTHI {}\n", tok, tok);
        let _ = parse_seq(&seq_snippet);

        // Construct malformed LEN snippet
        let len_snippet = format!("LEN NEW\nSURF 1\nRD {}\nTH {}\n", tok, tok);
        let _ = parse_len(&len_snippet);
    }
    println!("[FUZZ] All extreme float tokens parsed safely without panic");
    Ok(())
}

/// Stress-tests parsers with truncated prefixes of valid optical prescription files.
#[test]
fn test_fuzz_progressive_truncation() -> TestResult {
    let cooke_rx = cooke_triplet_f4_5()?;
    let zmx_source = COOKE_TRIPLET_F4_5_ZMX;
    let seq_source = write_seq(&CodeVSystem::from_optical_prescription(&cooke_rx));
    let oslo_source = write_len(&OsloSystem::from_optical_prescription(&cooke_rx));
    let json_source = OpticsProjectWorkspace::new("Cooke", cooke_rx).to_json()?;

    // Truncate at every length from 1 to total_len in 50 evenly spaced steps
    let steps = 50;

    for i in 1..=steps {
        let zmx_len = (zmx_source.len() * i) / steps;
        if let Some(slice) = zmx_source.get(..zmx_len) {
            let _ = parse_zmx(slice);
            let _ = parse_zmx_bytes(slice.as_bytes());
        }

        let seq_len = (seq_source.len() * i) / steps;
        if let Some(slice) = seq_source.get(..seq_len) {
            let _ = parse_seq(slice);
        }

        let oslo_len = (oslo_source.len() * i) / steps;
        if let Some(slice) = oslo_source.get(..oslo_len) {
            let _ = parse_len(slice);
        }

        let json_len = (json_source.len() * i) / steps;
        if let Some(slice) = json_source.get(..json_len) {
            let _ = OpticsProjectWorkspace::from_json(slice);
        }
    }

    println!("[FUZZ] Progressive truncation sweep completed without panic");
    Ok(())
}

/// Stress-tests parsers with random bit-flips, byte insertions, and random deletions.
#[test]
fn test_fuzz_random_mutations_and_bitflips() -> TestResult {
    let mut rng = FuzzRng::new(0x9988776655443322);
    let cooke_rx = cooke_triplet_f4_5()?;
    let base_zmx = COOKE_TRIPLET_F4_5_ZMX.as_bytes().to_vec();
    let base_seq = write_seq(&CodeVSystem::from_optical_prescription(&cooke_rx))
        .as_bytes()
        .to_vec();
    let base_len = write_len(&OsloSystem::from_optical_prescription(&cooke_rx))
        .as_bytes()
        .to_vec();

    // 200 fuzz iterations per format
    for _ in 0..200 {
        // Mutate ZMX
        let mut mutated_zmx = base_zmx.clone();
        mutate_bytes(&mut mutated_zmx, &mut rng);
        let _ = parse_zmx_bytes(&mutated_zmx);
        if let Ok(s) = std::str::from_utf8(&mutated_zmx) {
            let _ = parse_zmx(s);
        }

        // Mutate SEQ
        let mut mutated_seq = base_seq.clone();
        mutate_bytes(&mut mutated_seq, &mut rng);
        if let Ok(s) = std::str::from_utf8(&mutated_seq) {
            let _ = parse_seq(s);
        }

        // Mutate LEN
        let mut mutated_len = base_len.clone();
        mutate_bytes(&mut mutated_len, &mut rng);
        if let Ok(s) = std::str::from_utf8(&mutated_len) {
            let _ = parse_len(s);
        }
    }

    println!("[FUZZ] 600 random bitflip mutation runs passed cleanly without panic");
    Ok(())
}

/// Helper function to apply deterministic random mutations to a byte buffer.
fn mutate_bytes(bytes: &mut Vec<u8>, rng: &mut FuzzRng) {
    if bytes.is_empty() {
        return;
    }
    let mutations = rng.next_usize(5) + 1;
    for _ in 0..mutations {
        match rng.next_usize(4) {
            0 => {
                // Bit flip
                let idx = rng.next_usize(bytes.len());
                let bit = 1 << (rng.next_usize(8));
                bytes[idx] ^= bit;
            }
            1 => {
                // Byte replacement
                let idx = rng.next_usize(bytes.len());
                bytes[idx] = rng.next_u8();
            }
            2 => {
                // Byte insertion
                let idx = rng.next_usize(bytes.len());
                bytes.insert(idx, rng.next_u8());
            }
            3 => {
                // Byte deletion
                if bytes.len() > 1 {
                    let idx = rng.next_usize(bytes.len());
                    bytes.remove(idx);
                }
            }
            _ => {}
        }
    }
}
