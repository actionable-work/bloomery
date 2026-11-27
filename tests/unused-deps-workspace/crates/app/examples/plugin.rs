//! A library example: compiling it as a binary would fail without `main`.

pub fn plugin() {
    dep_example::used();
}
