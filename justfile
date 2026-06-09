# Dev task runner (https://github.com/casey/just). Mirrors the CI gates so they reproduce locally.
# Install once: `cargo install just`.

# Run the desktop app (opens the Slint window).
run:
    cargo run -p steadyinvest-app

# Run the whole test suite (incl. the cross-OS determinism-hash test in core).
test:
    cargo test --all

# Format + lint exactly as CI does.
lint:
    cargo fmt --all --check
    cargo clippy --all-targets --all-features -- -D warnings

# Full local CI: lint + test + license audit (needs `cargo install cargo-deny`).
ci: lint test
    cargo deny check

# Placeholder for the Week-1 charting go/no-go spike (Story 1.5 — native Slint <100 ms recolor).
spike:
    @echo "Week-1 spike harness lands in Story 1.5 (native-Slint draggable judgment line, <100 ms recolor)."
