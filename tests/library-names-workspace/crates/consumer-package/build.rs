fn main() {
    assert_eq!(build_alias::value(), 42);
    println!("cargo:rustc-env=NAMING_BUILD=42");
}
