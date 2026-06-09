//! Slint build wiring (mirrors the official Slint Rust template). Compiles the `.slint` UI into
//! generated Rust consumed by `slint::include_modules!()` in `main.rs`.
fn main() {
    slint_build::compile("ui/app.slint").expect("Slint UI compilation failed");
}
