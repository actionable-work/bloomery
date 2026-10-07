//! Workspace dependency used to prove that PGO rebuilds the binary's
//! workspace dependency closure with profile generation and use. It depends on
//! a registry crate so tests can distinguish workspace-scope from all-scope PGO.

pub fn offset() -> u64 {
    7
}

pub fn render(value: i64) -> String {
    let mut buffer = itoa::Buffer::new();
    buffer.format(value).to_owned()
}
