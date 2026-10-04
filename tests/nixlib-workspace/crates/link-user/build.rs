fn main() {
    let seen = std::env::var("DEP_LNK_KEY").unwrap_or_default();
    if seen == "value" {
        println!("cargo:rustc-cfg=dep_lnk_seen");
    }
}
