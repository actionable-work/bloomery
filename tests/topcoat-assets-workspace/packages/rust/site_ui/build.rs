fn main() {
    println!("cargo:rerun-if-env-changed=TAILWIND_CLI");
    println!("cargo:rerun-if-changed=src");
    topcoat::tailwind::BuildConfig::new()
        .executable_env("TAILWIND_CLI")
        .cwd("src")
        .render()
        .expect("render the Tailwind stylesheet");
}
