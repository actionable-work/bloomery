{lib}: let
  licenses = import ./licenses.nix {inherit lib;};
in {
  testSpdxBooleanPrecedenceAndParentheses = {
    expr = {
      andBeforeOr = licenses.parse "MIT OR Apache-2.0 AND BSD-3-Clause";
      explicitGrouping = licenses.parse "(MIT OR Apache-2.0) AND BSD-3-Clause";
    };
    expected = {
      andBeforeOr = {
        type = "OR";
        left = {
          type = "id";
          value = "MIT";
        };
        right = {
          type = "AND";
          left = {
            type = "id";
            value = "Apache-2.0";
          };
          right = {
            type = "id";
            value = "BSD-3-Clause";
          };
        };
      };
      explicitGrouping = {
        type = "AND";
        left = {
          type = "OR";
          left = {
            type = "id";
            value = "MIT";
          };
          right = {
            type = "id";
            value = "Apache-2.0";
          };
        };
        right = {
          type = "id";
          value = "BSD-3-Clause";
        };
      };
    };
  };
  testSpdxAllowDenyAndWithPairEvaluation = {
    expr = {
      choosesAllowedOrBranch = licenses.evaluate {
        expression = "MIT OR Apache-2.0";
        allow = ["Apache-2.0"];
      };
      requiresEveryAndBranch =
        !(licenses.evaluate {
          expression = "MIT AND Apache-2.0";
          allow = ["MIT"];
        });
      acceptsWithPair = licenses.evaluate {
        expression = "GPL-2.0-only WITH Classpath-exception-2.0";
        allow = ["GPL-2.0-only WITH Classpath-exception-2.0"];
      };
      deniesMatchingBranch =
        !(licenses.evaluate {
          expression = "MIT";
          deny = ["MIT"];
        });
      rejectsUnlistedExpression =
        !(licenses.evaluate {
          expression = "MIT OR Apache-2.0";
          allow = ["BSD-3-Clause"];
        });
    };
    expected = {
      choosesAllowedOrBranch = true;
      requiresEveryAndBranch = true;
      acceptsWithPair = true;
      deniesMatchingBranch = true;
      rejectsUnlistedExpression = true;
    };
  };
  testMalformedSpdxExpressionsAreRejected = {
    expr = map licenses.validExpression ["" "MIT OR" "MIT WITH" "MIT AND (Apache-2.0"];
    expected = [false false false false];
  };
}
