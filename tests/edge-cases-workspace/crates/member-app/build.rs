fn main() {
    // Emit modern cargo:: directive
    println!("cargo::rustc-cfg=modern_directive_active");
}
