## What and why

<!-- What does this change, and why? Link the issue if there is one. -->

## How it was tested

<!-- Tests added, and how you checked the behaviour (cargo test, desktop UI, CLI, screenshots). -->

## Checklist

- [ ] Quality gates pass (`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo build -p optics-desktop`).
- [ ] No panics in shipped code: no `unwrap()`, `expect()`, `panic!`, `todo!` or `unimplemented!` outside tests.
- [ ] Clean-room compliance: all algorithms derive strictly from public academic physics literature / ISO 10110. Nothing reverse-engineered or copied from commercial optical CAD (Zemax/Code V) or GPL/AGPL code (see [`AGENTS.md`](file:///Users/alexandrucrisan/Projects/optics-lab/AGENTS.md)).
- [ ] Every new asset file has a row in [`ASSETS.md`](file:///Users/alexandrucrisan/Projects/optics-lab/ASSETS.md).
