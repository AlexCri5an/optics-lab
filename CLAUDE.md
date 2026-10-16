# OpticsLab: working rules (read at the start of EVERY session)

## Project
OpticsLab is an open-source educational optics tool (Rust workspace, egui, optional wgpu later). The plan is in TODO.md (source of truth). Quality matters more than speed.

## Process (every task group)
1. Read TODO.md and the relevant docs in research/reverse_engineering/ before coding.
2. Restate the goal and acceptance criteria in 5 lines or less. Work ONLY on the requested group; list improvement ideas in the report instead of implementing them.
3. Work in small steps on a branch named `slice-x/short-name`; commit after each logical unit using conventional commits (feat:, fix:, test:, docs:, refactor:, chore:).
4. Ask before: adding dependencies (state license and size impact), changing public APIs, choosing between non-obvious alternatives.
5. If the plan or my instruction is physically, numerically or technically wrong or inconsistent, say so BEFORE coding.
6. At the end update TODO.md (tick only what is truly done and tested), CHANGELOG.md, and rustdoc.
## No publishing
- Do NOT create tags, GitHub Releases, crates.io publishes, GitHub Pages deployments, release/deploy workflows, or Homebrew formulas, and do not make the repository public, unless I explicitly ask in the current session.
- Local verification is fine and required: `trunk serve` / `trunk build` locally, `cargo run`, and the CI quality gates (fmt, clippy, test, wasm32 build check). CI may build and test, but must not deploy or publish anything.
- Packaging (.app/.dmg, installers, binaries for distribution) is out of scope until the final Release phase.

## Quality gates (all must pass before you report)
- cargo fmt --all -- --check
- cargo clippy --all --all-targets -- -D warnings
- cargo test --all
- cargo build --target wasm32-unknown-unknown -p optics-web
- cargo doc --no-deps with no warnings (once docs exist)
- No unsafe, no unwrap/expect/panic in non-test code, no leftover debug prints, no dead code, no TODO comments without a TODO.md reference.

## Correctness rules
- Units: nm at public APIs, micrometers inside dispersion formulas, degrees only at UI/CLI boundaries, radians internally. f64 on CPU, f32 on GPU; CPU-vs-GPU comparisons use stated tolerances.
- Never invent numeric constants, tables or reference values. Take them from the repo's data, from cited official sources (add the URL/standard in a comment), or derive them analytically inside the test and explain how.
- Every physics function gets: rustdoc with the formula, at least one analytic test, edge cases (grazing, normal incidence, TIR, zero-length vectors, NaN/inf inputs).
- Tests assert behavior, not implementation details; no test that only checks "does not panic" for numerical code.

## Architecture rules
- Pure logic (math, scene building, hit testing, formatting, view transforms) lives in testable functions/crates; egui/wgpu code only wires them.
- optics-core must not depend on optics-glass, optics-render or any UI/GPU crate.
- Keep the WASM build working at all times.

## UI/UX quality bar
- Labels, buttons, menu items in sentence case; no exclamation marks; errors say what happened and what to do; no raw error strings shown to users.
- Text must not overlap or clip at the minimum window size (900x600) or at 1x/2x DPI; text/background contrast of theme tokens must be at least WCAG AA (4.5:1), verified by a unit test that computes the ratio.
- Every interactive element has hover feedback and, where relevant, a tooltip; empty states give a hint; extreme inputs (slider limits, TIR everywhere, no ray escaping) are handled gracefully.
- Interactions must stay smooth: re-trace for a normal scene in well under one frame (report measured ms).

## Honesty rules
- Never claim the UI "looks good" or "works" unless you ran it. If you cannot run a GUI, say so explicitly and list what remains unverified.
- Report format: (1) summary, (2) files changed, (3) tests added (names), (4) gate results, (5) deviations from the request, (6) known issues, (7) proposed next step with acceptance criteria. Then stop and wait.


## UI rules (added)
- Visual style is defined in docs/ui-style.md ("Classic desktop"); do not deviate without updating that doc.
- wgpu is allowed as a dev-dependency of optics-ui (snapshot tests only). Runtime wgpu in optics-ui only behind the Slice C GPU viewport.
- No decorative or fake data anywhere in the UI. Every control either works, or is disabled with a tooltip saying why. Empty states say what is missing.
- Every UI change must be verified by snapshot images that you open and look at. Never claim a visual result you did not see.
- Icons are real assets (font or SVG files) with licenses listed in THIRD_PARTY_NOTICES.md. No icons drawn from ASCII/Unicode characters, no emoji, no Painter-drawn icons.