pub fn marker() {
    println!("cargo:rustc-env=ISOLATION_HARNESS_BUILD=1");
}
