fn main() {
    println!(
        "{} {}",
        symlink_entry::message(),
        env!("SYMLINK_BUILD_MARKER")
    );
}
