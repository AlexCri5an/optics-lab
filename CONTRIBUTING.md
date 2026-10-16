# Contributing to OpticsLab

Thank you for your interest in contributing to **OpticsLab**! We are building an open-source optical design and numerical simulation backend in Rust.

---

## 1. Code of Conduct

All contributors and maintainers are expected to adhere to our [Code of Conduct](CODE_OF_CONDUCT.md). Please read it before participating.

---

## 2. Core Engineering Principles

1. **Correctness Before Speed**: Optical algorithms must be mathematically verified before performance optimization. Where analytical solutions exist, tests must compare numerical traces against closed-form expressions.
2. **Zero-Panic Policy**: Library crates (`optics-core`, `optics-spectral`, `optics-glass`, `optics-bvh`, `optics-zemax`, `optics-render`) strictly deny `unwrap_used` and `expect_used`. All recoverable errors must use typed errors via `thiserror`.
3. **Finite Inputs and Domain Validation**: All public functions must validate non-finite inputs (`NaN`, $\pm\infty$) and physical surface domains (apertures, square roots, radical boundaries).
4. **Reproducibility and Determinism**: Stochastic routines (e.g. Monte Carlo ray tracing, random optimizer restarts) must accept an explicit seed and produce deterministic outputs.
5. **Serializable UI Boundary**: Everything consumable by frontend UI layers must implement `serde::Serialize`, `serde::Deserialize`, `Clone`, and `Debug`.

---

## 3. Development Workflow

### Prerequisites
- Rust 1.87 or newer (stable toolchain)
- (Optional) Vulkan / Metal / DirectX 12 GPU for rendering backend

### Pre-Commit Quality Gates
Every contribution must pass all four quality gates:

```bash
# 1. Format check
cargo fmt --all -- --check

# 2. Strict lints (zero warnings allowed)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 3. Full test suite
cargo test --workspace

# 4. Documentation build
cargo doc --workspace --no-deps
```

---

## 4. Submitting Pull Requests

1. Fork the repository and create a feature branch (`feature/your-feature-name` or `fix/issue-id`).
2. Write unit and regression tests for all added functionality.
3. Keep commit messages clear and conventional:
   - `feat(optics-core): add even asphere surface sag and normal`
   - `fix(optics-bvh): reserve consecutive child nodes for binary traversal`
   - `docs(audit): update feature matrix for phase 0`
4. Ensure all quality gates pass locally before submitting.

---

## 5. Licensing of Contributions

Unless explicitly stated otherwise, any contribution intentionally submitted for inclusion in OpticsLab shall be dual-licensed under both the **MIT License** and the **Apache License (Version 2.0)**, without any additional terms or conditions.
