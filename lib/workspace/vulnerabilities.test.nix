{lib}: let
  vulnerabilities = import ./vulnerabilities.nix {inherit lib;};
  advisories = vulnerabilities.loadDatabase ./fixtures/rustsec-db;
  package = name: version: source: {
    inherit name version source;
    isRegistry = lib.hasPrefix "registry+" source;
  };
  entry = name: version: source: {
    id = "${name}-${version}";
    inherit name version;
    package = package name version source;
    pathText = "app (1.0.0) -> ${name} (${version})";
  };
  config = outcome: {
    ignoreAdvisories = [];
    informational = {
      unmaintained = outcome;
      unsound = outcome;
      notice = "warn";
      unknown = "warn";
    };
  };
  scan = entries: cfg:
    vulnerabilities.scan {
      memberId = "app-1.0.0";
      inherit entries advisories;
      config = cfg;
    };
  badSelector = vulnerabilities.extractFrontMatter {
    path = "bad-selector.md";
    contents = ''
      ---
      id = "RUSTSEC-2024-9999"
      package = "crate-a"
      [versions]
      patched = [">> 1.0.0"]
      ---
      # bad selector
    '';
  };
  missingVersions = vulnerabilities.extractFrontMatter {
    path = "missing-versions.md";
    contents = ''
      ---
      id = "RUSTSEC-2024-9998"
      package = "crate-a"
      ---
      # missing versions table
    '';
  };
in {
  testLoadsRustSecV3AndV4FrontMatter = {
    expr = map (advisory: advisory.id) advisories;
    expected = ["RUSTSEC-2024-1001" "RUSTSEC-2024-1002" "RUSTSEC-2024-1003" "RUSTSEC-2024-1004"];
  };
  testCargoSemverSelectorsMatchRanges = {
    expr = {
      caretIncludesCompatibleMinor = vulnerabilities.matchesRange "1.4.0" "^1.2.3";
      caretExcludesNextMajor = !(vulnerabilities.matchesRange "2.0.0" "^1.2.3");
      zeroMajorCaretStopsAtMinor = !(vulnerabilities.matchesRange "0.3.0" "^0.2.3");
      wildcardZeroMajorCoversMinorRange = vulnerabilities.matchesRange "0.9.0" "0.*";
      commaRangeMatches = vulnerabilities.matchesRange "1.2.4" ">= 1.2.3, < 1.3.0";
      prereleaseSortsBeforeRelease = !(vulnerabilities.matchesRange "1.2.3-alpha.1" ">= 1.2.3");
      prereleaseAtLaterPatchRequiresExplicitPrereleaseBound = !(vulnerabilities.matchesRange "1.2.4-alpha.1" ">= 1.2.3");
      explicitlySelectedPrereleaseMatches = vulnerabilities.matchesRange "1.2.3-beta.1" ">= 1.2.3-alpha.1, < 1.2.3";
      partialComparatorMatchesSameMinor = vulnerabilities.matchesRange "1.2.8" ">= 1.2";
      partialGreaterComparatorStartsAtNextMinor = !(vulnerabilities.matchesRange "1.2.8" "> 1.2");
      caretRejectsEarlierPrerelease = !(vulnerabilities.matchesRange "1.2.3-alpha.1" "^1.2.3-beta.1");
      caretAcceptsLaterPrerelease = vulnerabilities.matchesRange "1.2.3-beta.2" "^1.2.3-beta.1";
    };
    expected = {
      caretIncludesCompatibleMinor = true;
      caretExcludesNextMajor = true;
      zeroMajorCaretStopsAtMinor = true;
      wildcardZeroMajorCoversMinorRange = true;
      commaRangeMatches = true;
      prereleaseSortsBeforeRelease = true;
      prereleaseAtLaterPatchRequiresExplicitPrereleaseBound = true;
      explicitlySelectedPrereleaseMatches = true;
      partialComparatorMatchesSameMinor = true;
      partialGreaterComparatorStartsAtNextMinor = true;
      caretRejectsEarlierPrerelease = true;
      caretAcceptsLaterPrerelease = true;
    };
  };
  testAdvisoriesMatchAffectedCratesIoVersions = {
    expr = map (finding: finding.advisoryId) (scan [
      (entry "crate-a" "1.2.2" "registry+https://github.com/rust-lang/crates.io-index")
      (entry "crate-a" "1.2.3" "registry+https://github.com/rust-lang/crates.io-index")
      (entry "crate-b" "1.0.0" "registry+https://github.com/rust-lang/crates.io-index")
      (entry "crate-b" "2.0.0" "registry+https://github.com/rust-lang/crates.io-index")
    ] (config "warn"));
    expected = ["RUSTSEC-2024-1001" "RUSTSEC-2024-1002"];
  };
  testAdvisoriesSkipNonCratesIoAndWithdrawnRecords = {
    expr = {
      ignoredPackages = scan [
        (entry "crate-a" "1.0.0" "git+https://example.com/crate-a")
        (entry "crate-a" "1.0.0" "registry+https://example.com/index")
        (entry "crate-c" "0.9.0" "registry+https://github.com/rust-lang/crates.io-index")
      ] (config "warn");
      acceptsSparseCratesIo = vulnerabilities.isCratesIoPackage (package "crate-a" "1.0.0" "sparse+https://index.crates.io/");
    };
    expected = {
      ignoredPackages = [];
      acceptsSparseCratesIo = true;
    };
  };
  testIgnoredAdvisoryIdsSuppressAllFindings = {
    expr = scan [(entry "crate-a" "1.0.0" "registry+https://github.com/rust-lang/crates.io-index")] ((config "deny") // {ignoreAdvisories = ["RUSTSEC-2024-1001"];});
    expected = [];
  };
  testInformationalOutcomesAndMostSevereCategoryWin = {
    expr = {
      warnsByDefault = (builtins.head (scan [(entry "crate-b" "1.0.0" "registry+https://github.com/rust-lang/crates.io-index")] (config "warn"))).outcome;
      deniesWhenConfigured = (builtins.head (scan [(entry "crate-b" "1.0.0" "registry+https://github.com/rust-lang/crates.io-index")] (config "deny"))).outcome;
      suppressesWhenIgnored = scan [(entry "crate-b" "1.0.0" "registry+https://github.com/rust-lang/crates.io-index")] (config "ignore");
      choosesMostSevere = (builtins.head (scan [(entry "crate-d" "2.0.0" "registry+https://github.com/rust-lang/crates.io-index")] ((config "warn") // {informational.unsound = "deny";}))).outcome;
    };
    expected = {
      warnsByDefault = "warn";
      deniesWhenConfigured = "deny";
      suppressesWhenIgnored = [];
      choosesMostSevere = "deny";
    };
  };
  testUnsupportedAdvisoryDataFailsClosed = {
    expr = {
      malformedRangeFails =
        !(builtins.tryEval (builtins.deepSeq (vulnerabilities.scan {
            memberId = "app-1.0.0";
            entries = [(entry "crate-a" "1.0.0" "registry+https://github.com/rust-lang/crates.io-index")];
            advisories = [badSelector];
            config = config "warn";
          })
          true)).success;
      missingVersionsFails = !(builtins.tryEval (builtins.deepSeq missingVersions true)).success;
      badRangeFailsWhenMatched =
        !(builtins.tryEval (builtins.deepSeq (vulnerabilities.scan {
            memberId = "app-1.0.0";
            entries = [(entry "crate-a" "1.0.0" "registry+https://github.com/rust-lang/crates.io-index")];
            advisories = [badSelector];
            config = config "warn";
          })
          true)).success;
      badRangeFailsWithoutMatchingPackage =
        !(builtins.tryEval (builtins.deepSeq (vulnerabilities.scan {
            memberId = "app-1.0.0";
            entries = [];
            advisories = [badSelector];
            config = config "warn";
          })
          true)).success;
    };
    expected = {
      malformedRangeFails = true;
      missingVersionsFails = true;
      badRangeFailsWhenMatched = true;
      badRangeFailsWithoutMatchingPackage = true;
    };
  };
}
