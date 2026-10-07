use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn run(command: &mut Command) {
    let status = command.status().expect("failed to spawn command");
    assert!(status.success(), "command failed: {command:?}");
}

fn main() {
    println!("cargo:key=value");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by the builder"));
    let source = out_dir.join("links_sys_native.c");
    fs::write(
        &source,
        r#"
int links_sys_static_value(void) { return 7; }
int links_sys_shared_value(void) { return 5; }
"#,
    )
    .expect("write native source");

    let object = out_dir.join("links_sys_native.o");
    run(Command::new("cc")
        .arg("-c")
        .arg("-fPIC")
        .arg("-o")
        .arg(&object)
        .arg(&source));
    run(Command::new("ar")
        .arg("rcs")
        .arg(out_dir.join("liblinks_sys_static.a"))
        .arg(&object));
    run(Command::new("cc")
        .arg("-shared")
        .arg("-fPIC")
        .arg("-o")
        .arg(out_dir.join("liblinks_sys_shared.so"))
        .arg(&source));

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=links_sys_static");
    println!("cargo:rustc-link-lib=dylib=links_sys_shared");
}
