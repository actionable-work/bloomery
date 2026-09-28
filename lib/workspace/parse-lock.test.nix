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

  testParseGitDepString = {
    expr = parseLock.parseDepString "my-git-crate 0.3.0 (git+https://github.com/foo/bar.git?branch=main#0123456789abcdef)";
    expected = {
      name = "my-git-crate";
      version = "0.3.0";
      source = "git+https://github.com/foo/bar.git?branch=main#0123456789abcdef";
      raw = "my-git-crate 0.3.0 (git+https://github.com/foo/bar.git?branch=main#0123456789abcdef)";
    };
  };

  testCrlfHashNormalization = let
    crlfContent = "version = 3\r\n\r\n[[package]]\r\nname = \"foo\"\r\n";
    lfContent = "version = 3\n\n[[package]]\nname = \"foo\"\n";
    hashNormalize = s: builtins.hashString "sha256" (lib.replaceStrings ["\r\n"] ["\n"] s);
  in {
    expr = (hashNormalize crlfContent) == (hashNormalize lfContent);
    expected = true;
  };
}
