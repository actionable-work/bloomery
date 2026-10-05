{lib}: let
  policy = import ./manifest-policy.nix {inherit lib;};

  member = name: toml: {inherit name toml;};

  compliantWorkspaceDependencies = {
    lib-core = {
      path = "crates/lib-core";
      default-features = false;
    };
    itoa = {
      version = "1.0.18";
      default-features = false;
    };
  };

  compliantMember = member "app" {
    dependencies = {
      lib-core = {workspace = true;};
      itoa = {workspace = true;};
    };
  };

  evaluate = policy.evaluate;
in {
  testWorkspaceDependencyViolationsPassWhenFullyInherited = {
    expr =
      (evaluate {
        workspaceDependencies = compliantWorkspaceDependencies;
        members = [compliantMember];
      })
      .workspaceDependencyViolations
      == [];
    expected = true;
  };

  testDefaultFeatureViolationsPassWhenFullyDisabled = {
    expr =
      (evaluate {
        workspaceDependencies = compliantWorkspaceDependencies;
        members = [compliantMember];
      })
      .defaultFeatureViolations
      == [];
    expected = true;
  };

  testBareVersionStringIsNotAWorkspaceDependency = {
    expr =
      builtins.length
      (evaluate {
        workspaceDependencies = compliantWorkspaceDependencies;
        members = [
          (member "app" {
            dependencies.itoa = "1.0.18";
          })
        ];
      })
      .workspaceDependencyViolations
      == 1;
    expected = true;
  };

  testInlinePathIsNotAWorkspaceDependency = {
    expr =
      builtins.length
      (evaluate {
        workspaceDependencies = compliantWorkspaceDependencies;
        members = [
          (member "app" {
            dependencies.lib-core = {path = "../lib-core";};
          })
        ];
      })
      .workspaceDependencyViolations
      == 1;
    expected = true;
  };

  testInlineVersionIsNotAWorkspaceDependency = {
    expr =
      builtins.length
      (evaluate {
        workspaceDependencies = compliantWorkspaceDependencies;
        members = [
          (member "app" {
            dependencies.itoa = {version = "1.0";};
          })
        ];
      })
      .workspaceDependencyViolations
      == 1;
    expected = true;
  };

  testWorkspaceEntryWithoutDisabledDefaultFeaturesFails = {
    expr =
      builtins.length
      (evaluate {
        workspaceDependencies = {
          itoa = {version = "1.0";};
        };
        members = [compliantMember];
      })
      .defaultFeatureViolations
      >= 1;
    expected = true;
  };

  testMemberOverrideEnablingDefaultFeaturesFails = {
    expr =
      builtins.length
      (evaluate {
        workspaceDependencies = compliantWorkspaceDependencies;
        members = [
          (member "app" {
            dependencies.itoa = {
              workspace = true;
              default-features = true;
            };
          })
        ];
      })
      .defaultFeatureViolations
      == 1;
    expected = true;
  };

  testUnderscoreDefaultFeaturesNormalizes = {
    expr =
      (evaluate {
        workspaceDependencies = {
          itoa = {
            version = "1.0";
            default_features = false;
          };
        };
        members = [
          (member "app" {
            dependencies.itoa = {
              workspace = true;
              default_features = false;
            };
          })
        ];
      })
      .defaultFeatureViolations
      == [];
    expected = true;
  };

  testUnderscoreBuildDependenciesTableIsCollected = {
    expr = let
      entries = policy.collectDependencyEntries {
        build_dependencies = {
          itoa = {workspace = true;};
        };
      };
    in {
      count = builtins.length entries;
      table = (builtins.head entries).table;
    };
    expected = {
      count = 1;
      table = "build-dependencies";
    };
  };

  testTargetSpecificDependenciesAreCollected = {
    expr = let
      entries = policy.collectDependencyEntries {
        target."cfg(unix)".dependencies.itoa = {workspace = true;};
      };
    in
      builtins.length entries == 1;
    expected = true;
  };

  testEffectiveDefaultFeaturesPrefersMemberThenWorkspace = {
    expr = {
      memberOverride = policy.effectiveDefaultFeatures compliantWorkspaceDependencies {
        name = "itoa";
        spec = {
          workspace = true;
          default-features = true;
        };
      };
      workspaceFallback = policy.effectiveDefaultFeatures compliantWorkspaceDependencies {
        name = "itoa";
        spec = {workspace = true;};
      };
      cargoDefault = policy.effectiveDefaultFeatures {} {
        name = "itoa";
        spec = {workspace = true;};
      };
    };
    expected = {
      memberOverride = true;
      workspaceFallback = false;
      cargoDefault = true;
    };
  };
}
