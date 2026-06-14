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

# Week-1 entry-regime go/no-go spike (Story 1.4 — dense editable grid + paste-a-column).
# Needs a display + a clipboard. Click a cell, copy a column of numbers elsewhere, press Ctrl+V.
spike-a:
    cargo run -p steadyinvest-app --example spike_a_grid

# Week-1 precision/determinism go/no-go spike (Story 1.6 — exact-decimal CAGR + pinned hash).
# Headless (no display needed); the measured deviations print to stderr.
spike-c:
    cargo test -p steadyinvest-core --test spike_c_cagr_precision -- --nocapture
