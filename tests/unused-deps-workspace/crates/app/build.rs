//! Emits a cfg from Cargo's build script environment; the library's only
//! normal use of `core-lib` depends on it.
fn main() {
    dep_build::used();
    std::env::var("CARGO_CFG_TARGET_POINTER_WIDTH").expect("Cargo's CARGO_CFG_* environment");
    println!("cargo::rustc-check-cfg=cfg(app_build_env)");
    println!("cargo::rustc-cfg=app_build_env");
}
