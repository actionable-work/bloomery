use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let root = templates_dir();
    println!("cargo:rerun-if-changed={}", root.display());

    let mut entries: Vec<(String, String, String)> = Vec::new();
    for name in ["basic", "axum", "topcoat"] {
        walk(&root, &root.join(name), &mut entries);
    }

    let mut generated = String::from("pub static TEMPLATES: &[(&str, &str, &str)] = &[\n");
    for (name, relative, content) in &entries {
        writeln!(generated, "    ({name:?}, {relative:?}, {content:?}),").unwrap();
    }
    generated.push_str("];\n");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by Cargo");
    fs::write(Path::new(&out_dir).join("templates.rs"), generated)
        .expect("write generated template table");
}

fn templates_dir() -> PathBuf {
    if let Some(dir) = env::var_os("BLOOMERY_TEMPLATES_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../templates")
}

fn walk(root: &Path, dir: &Path, entries: &mut Vec<(String, String, String)>) {
    let mut paths = fs::read_dir(dir)
        .expect("read template directory")
        .map(|entry| entry.expect("template directory entry").path())
        .collect::<Vec<_>>();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            walk(root, &path, entries);
            continue;
        }
        println!("cargo:rerun-if-changed={}", path.display());
        let full = path
            .strip_prefix(root)
            .expect("template path below catalog root")
            .to_string_lossy()
            .replace('\\', "/");
        let mut parts = full.splitn(2, '/');
        let name = parts.next().expect("template name").to_owned();
        let relative = parts.next().expect("template-relative path").to_owned();
        let content = fs::read_to_string(&path).expect("read template file");
        entries.push((name, relative, content));
    }
}
