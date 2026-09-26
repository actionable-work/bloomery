{lib}: let
  parseLock = import ./parse-lock.nix {inherit lib;};

  sampleLock = ''
    version = 3

    [[package]]
    name = "foo"
    version = "0.1.0"
    dependencies = [
      "bar 0.2.0",
    ]

    [[package]]
    name = "bar"
    version = "0.2.0"
    source = "registry+https://github.com/rust-lang/crates.io-index"
    checksum = "abcdef123456"
  '';

  parsed = parseLock.parseLock {lockContent = sampleLock;};
in {
  testParseDepString = {
    expr = parseLock.parseDepString "syn 2.0.119 (registry+https://crates.io)";
    expected = {
      name = "syn";
      version = "2.0.119";
      source = "registry+https://crates.io";
      raw = "syn 2.0.119 (registry+https://crates.io)";
    };
  };

  testParseLockPackages = {
    expr = {
      hasFoo = parsed.byName ? foo;
      hasBar = parsed.byName ? bar;
      fooIsWorkspace = (builtins.head parsed.byName.foo).isWorkspace;
      barIsRegistry = (builtins.head parsed.byName.bar).isRegistry;
    };
    expected = {
      hasFoo = true;
      hasBar = true;
      fooIsWorkspace = true;
      barIsRegistry = true;
    };
  };

  testParseLockResolvedDeps = {
    expr = (builtins.head parsed.byName.foo).depIds;
    expected = ["bar-0.2.0"];
  };
}
