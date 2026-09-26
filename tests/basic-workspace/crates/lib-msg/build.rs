fn main() {
    println!("cargo:rustc-cfg=bloomery_banner");
    println!("cargo:rustc-env=MSG_BANNER=[Bloomery System]");
}
