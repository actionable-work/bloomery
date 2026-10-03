# Nix scanner

## Reference metadata

Nix checks expose requirement IDs through a `passthru.bloomery` list. The
scanner evaluates the configured checks attribute and reads that metadata:

```nix
checks.x86_64-linux.auth-integration = pkgs.testers.runNixOSTest {
  name = "auth-integration";
  passthru.bloomery = [
    "CLI-EXAMPLE-AUTH-001"
    "CLI-EXAMPLE-AUTH-002"
  ];
};
```

For each configured system, the scanner performs the equivalent of a JSON
metadata evaluation against the configured checks attribute. It reads
`passthru.bloomery` values and emits the check attribute and system as source
context.

## Static boundary

Nix evaluation must not instantiate or execute check derivations. It must not
run test scripts or build outputs, and it must not use the evaluator to fetch
missing inputs. Evaluation failures are scanner diagnostics rather than an
empty result.
