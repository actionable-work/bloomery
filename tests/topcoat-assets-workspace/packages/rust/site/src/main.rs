use topcoat::asset::{Asset, asset};

/// Declared by the binary crate itself rather than a library dependency.
const ROBOTS: Asset = asset!("assets/robots.txt");

fn main() {
    std::hint::black_box(ROBOTS.id());
    site_server::serve();
}
