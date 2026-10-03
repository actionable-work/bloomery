# Nix scanner

## Reference metadata

Nix checks expose requirement IDs through a `passthru.bloomery` list of string
literals:

```nix
checks.x86_64-linux.auth-integration = pkgs.runCommand "auth-integration" {
  passthru.bloomery = [
    "CLI-EXAMPLE-AUTH-001"
    "CLI-EXAMPLE-AUTH-002"
  ];
} ''
  run-auth-integration-tests
'';
```

The scanner walks the repository-relative `scanners.nix.paths` globs and
statically tokenizes matching Nix source files. It extracts literal IDs from
both `passthru.bloomery = [ ... ];` and a nested
`passthru = { bloomery = [ ... ]; };` attribute set, retaining each string's
source file and line. Comments and Nix strings outside the metadata list do not
create evidence.

Metadata should contain literal, contract-valid requirement IDs. Dynamically
computed IDs cannot be discovered statically and are not evidence.

## Static boundary

The Nix scanner reads source text only. It does not invoke `nix eval`, evaluate
flakes, instantiate or build derivations, fetch inputs, or execute check
scripts. File-read and malformed metadata diagnostics are reported as scanner
errors rather than silently treated as empty evidence.
