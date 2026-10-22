{lib}: let
  licenses = import ./licenses.nix {inherit lib;};
  policy = import ./dependency-policy.nix {inherit lib licenses;};
  packages = {
    "app-1.0.0" = {
      name = "app";
      version = "1.0.0";
      isWorkspace = true;
    };
    "middle-1.0.0" = {
      name = "middle";
      version = "1.0.0";
      isWorkspace = false;
    };
    "leaf-2.0.0" = {
      name = "leaf";
      version = "2.0.0";
      isWorkspace = false;
    };
    "unlicensed-1.0.0" = {
      name = "unlicensed";
      version = "1.0.0";
      isWorkspace = false;
    };
  };
  context = {
    "app-1.0.0" = {dependencies = ["middle-1.0.0" "unlicensed-1.0.0"];};
    "middle-1.0.0" = {dependencies = ["leaf-2.0.0"];};
    "leaf-2.0.0" = {dependencies = [];};
    "unlicensed-1.0.0" = {dependencies = [];};
  };
  lockPackages = {
    "middle-1.0.0" = {license = "MIT";};
    "leaf-2.0.0" = {license = "GPL-3.0-only";};
  };
  entries = policy.packageEntries {
    memberId = "app-1.0.0";
    inherit context packages lockPackages;
  };
in {
  testPolicyWalksTransitiveContextAndExcludesMember = {
    expr = map (entry: entry.id) entries;
    expected = ["leaf-2.0.0" "middle-1.0.0" "unlicensed-1.0.0"];
  };
  testPolicyReportsExampleDependencyPath = {
    expr = (builtins.head (builtins.filter (entry: entry.id == "leaf-2.0.0") entries)).pathText;
    expected = "app (1.0.0) -> middle (1.0.0) -> leaf (2.0.0)";
  };
  testPackageBanPrecedenceAndVersionSelectors = {
    expr = map (entry: entry.id) (policy.bans {
      memberId = "app-1.0.0";
      inherit context packages lockPackages;
      config = {
        allow = ["middle"];
        deny = ["middle@1.0.0"];
        exceptions = ["middle@1.0.0"];
      };
    });
    expected = ["leaf-2.0.0" "unlicensed-1.0.0"];
  };
  testPackageBanReportsDeniedTransitivePackage = {
    expr = map (entry: entry.id) (policy.bans {
      memberId = "app-1.0.0";
      inherit context packages lockPackages;
      config = {
        allow = ["middle" "leaf"];
        deny = ["leaf@2.0.0"];
        exceptions = [];
      };
    });
    expected = ["leaf-2.0.0" "unlicensed-1.0.0"];
  };
  testLicenseOverrideMissingMetadataAndExceptions = {
    expr = map (entry: entry.kind) (policy.licensesCheck {
      memberId = "app-1.0.0";
      inherit context packages lockPackages;
      config = {
        allow = ["MIT"];
        deny = [];
        exceptions = ["leaf"];
        overrides = {unlicensed = "MIT OR Apache-2.0";};
      };
    });
    expected = [];
  };
  testLicenseOverrideTakesPrecedenceAndMissingExpressionFails = {
    expr =
      map (entry: {
        inherit (entry) name kind;
        expression = entry.licenseExpression or null;
      }) (policy.licensesCheck {
        memberId = "app-1.0.0";
        inherit context packages lockPackages;
        config = {
          allow = ["MIT"];
          deny = [];
          exceptions = [];
          overrides = {"leaf@2.0.0" = "MIT";};
        };
      });
    expected = [
      {
        name = "unlicensed";
        kind = "missing-license";
        expression = null;
      }
    ];
  };
}
