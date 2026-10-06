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

The scanner walks the repository-relative `scanners.nix.paths` globs plus the
`scanners.nix.testPaths` test globs and
statically tokenizes matching Nix source files. It extracts literal IDs from
both `passthru.bloomery = [ ... ];` and a nested
`passthru = { bloomery = [ ... ]; };` attribute set, retaining each string's
source file and line. Comments and Nix strings outside the metadata list do not
create evidence.

Metadata should contain literal, contract-valid requirement IDs. Dynamically
computed IDs cannot be discovered statically and are not evidence.

## Test discovery

Nix test files are selected by the configured `scanners.nix.testPaths` globs,
which default to `["**/*.test.nix"]`. When the Nix scanner is enabled,
discovery enumerates the matching repository-relative files and treats each as
a test site named by its file stem. An empty `testPaths` list disables Nix
test-file discovery. The file's
`passthru.bloomery` metadata supplies the site's references: a `.test.nix` file
with no metadata is an untied graduation candidate, and one with metadata is
tied. Discovery never evaluates the file, so it recognizes tests by configured
glob and metadata rather than by evaluating `checks.*` attributes. See the
[test discovery contract](test-discovery.md#nix-tests).

## Static boundary

The Nix scanner reads source text only. It does not invoke `nix eval`, evaluate
flakes, instantiate or build derivations, fetch inputs, or execute check
scripts. File-read and malformed metadata diagnostics are reported as scanner
errors rather than silently treated as empty evidence.
