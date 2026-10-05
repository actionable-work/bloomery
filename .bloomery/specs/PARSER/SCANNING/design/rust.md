# Rust scanner

## Reference syntax

Rust tests associate a requirement with a function using a compile-time
disabled attribute nested in `cfg_attr`:

```rust
#[cfg_attr(any(), bloomery("CLI-EXAMPLE-AUTH-001"))]
#[tokio::test]
async fn rejects_invalid_credentials() {
    // test body is never executed by Bloomery
}
```

`any()` is false, so the `bloomery` attribute is never applied and the source
compiles without a proc-macro crate. Evidence tags therefore add no dependency
to workspace or generated template crates.

The scanner parses configured `.rs` files with `syn`. It visits function
declarations, inspects their `cfg_attr` attributes, and extracts the string
argument of a nested `bloomery(...)` meta item. The tag is intended for test
functions, including functions carrying `#[test]` or `#[tokio::test]`; a
malformed nested argument or a non-string value is a source diagnostic.

## Static boundary

The scanner never compiles or executes Rust. It does not infer requirement IDs
from function names, comments, or test bodies. The source span of the
`cfg_attr` attribute is retained for orphan and illegal-manual diagnostics.
