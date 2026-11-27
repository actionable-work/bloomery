//! Uses only `core-lib` outside tests. The parameter shares a dependency's
//! crate name without referencing the crate, and the lint allow below must not
//! hide the unused dependencies.
#![allow(unused_crate_dependencies)]

/// Only this documentation test uses `dep-doc`:
///
/// ```
/// dep_doc::used();
/// ```
pub fn run(dep_unused: u8) -> u8 {
    #[cfg(app_build_env)]
    core_lib::used();
    dep_unused
}

#[cfg(test)]
mod tests {
    #[test]
    fn uses_a_normal_dependency_only_in_tests() {
        dep_test_only::used();
    }
}
