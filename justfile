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

# Week-1 charting go/no-go spike (Story 1.5 — native-Slint draggable judgment line + <100 ms recolor).
# Needs a display; drag the white line and watch the zone bar recolor. Logs recompute µs to stderr.
spike-b:
    cargo run -p steadyinvest-app --example spike_b_chart
