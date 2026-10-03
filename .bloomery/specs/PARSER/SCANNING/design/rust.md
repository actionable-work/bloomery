# Rust scanner

## Reference syntax

Rust tests associate a requirement with a function using an outer attribute:

```rust
#[bloomery("CLI-EXAMPLE-AUTH-001")]
#[tokio::test]
async fn rejects_invalid_credentials() {
    // test body is never executed by Bloomery
}
```

The scanner parses configured `.rs` files with `syn`. It visits function
declarations and extracts string arguments from `bloomery(...)` attributes.
The attribute is intended for test functions, including functions carrying
`#[test]` or `#[tokio::test]`; a malformed argument or a non-string value is a
source diagnostic.

## Static boundary

The scanner never compiles or executes Rust. It does not infer requirement IDs
from function names, comments, or test bodies. The source span of the
attribute is retained for orphan and illegal-manual diagnostics.
