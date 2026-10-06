# Test discovery

Test discovery is the static counterpart to reference extraction. Instead of
asking which requirement IDs a source references, it asks which tests the
configured source declares, whether or not they carry a requirement reference.
The resulting test sites feed the `spec candidates` projection.

## Model

Each discovered site is:

```text
TestSite {
  scanner: rust | playwright | nix
  name: Option<String>
  location: SourceLocation
  references: Vec<RequirementId>
}
```

The `references` vector holds the requirement IDs extracted from the same site
by the existing scanner rules. A site with an empty vector is untied and becomes
a graduation candidate. A site may not appear twice; discovery is keyed by
source location.

Requirement references remain owned by the existing scanner contracts. Test
discovery attaches them to the site rather than introducing a second extraction
path.

## Rust tests

The Rust scanner visits function declarations and identifies a test function
when it carries an attribute whose path is `test` or whose final segment is
`test`, matching the existing guide used for evidence tags. The test name is the
function identifier. `cfg_attr(any(), bloomery("..."))` tags on the function
populate `references` exactly as they do for reference extraction.

A function without a test attribute is not a site, even when it carries a
`bloomery` tag; the tag rules already restrict evidence to test functions.
Non-string bloomery arguments remain diagnostics.

## Playwright tests

The Playwright scanner visits `test(...)` and `test.describe(...)` call
expressions. The test name is the first string argument when it is a plain
string literal. Tags in the options object beginning with the configured
`tag_prefix` populate `references` after the prefix is removed.

Calls that are not Playwright tests and string tags outside a test call are not
sites.

## Nix tests

Nix test files are selected by the configured `scanners.nix.testPaths` globs,
which default to `["**/*.test.nix"]`. When the Nix scanner is enabled,
discovery enumerates the matching repository-relative files and treats each as
a test site named by its file stem. An empty `testPaths` list disables Nix
test-file discovery while `scanners.nix.paths` still supplies references.

The scanner tokenizes the same file for `passthru.bloomery` metadata. A file
that declares at least one literal requirement ID is tied; a file that declares
none is an untied candidate. Because discovery never evaluates Nix, test files
are recognized by configured glob and by metadata rather than by evaluating
`checks.*` attributes.

## Static boundary

Discovery reads configured files and parses them; it does not compile or execute
Rust, run Playwright, evaluate flakes, instantiate derivations, or open network
connections. A file that fails to parse is reported as a scanner diagnostic and
does not contribute partial sites, matching reference extraction.