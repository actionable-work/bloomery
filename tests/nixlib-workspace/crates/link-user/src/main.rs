fn main() {
    #[cfg(dep_lnk_seen)]
    println!("dep seen");
    #[cfg(not(dep_lnk_seen))]
    println!("dep missing");
}
