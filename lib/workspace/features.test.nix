{ lib }:

let
  features = import ./features.nix { inherit lib; };
in {
  testExtractFeaturesExplicit = {
    expr = features.extractFeaturesFromToml {
      dependencies = {
        serde = { version = "1.0"; features = [ "derive" ]; default-features = false; };
        tokio = { version = "1.0"; features = [ "sync" ]; };
        plain = "0.1";
      };
    };
    expected = {
      serde = [ "derive" ];
      tokio = [ "sync" "default" ];
      plain = [ "default" ];
    };
  };

  testExtractFeaturesWorkspace = {
    expr = features.extractFeaturesFromToml {
      workspace.dependencies = {
        axum = { version = "0.7"; features = [ "json" "ws" ]; };
      };
    };
    expected = {
      axum = [ "json" "ws" "default" ];
    };
  };

  testExtractFeaturesFromFeatureTable = {
    expr = features.extractFeaturesFromToml {
      dependencies = {
        hyper = { version = "1.0"; default-features = false; };
      };
      features = {
        server = [ "hyper/server" "hyper/http1" ];
      };
    };
    expected = {
      hyper = [ "server" "http1" ];
    };
  };

  testResolveFeaturesUnified = {
    expr =
      let
        res = features.resolveFeatures {
          root = ./.;
          cargoTomlPath = ./does-not-exist-Cargo.toml;
          unifyFeatures = true;
        };
      in res ? "tokio" || (builtins.removeAttrs res [ "__activeDeps" ] == {});
    expected = true;
  };

  testWeakDependencyNotActivated = {
    expr =
      let
        mockTomls = {
          "topcoat" = {
            dependencies = {
              "topcoat-core" = { version = "0.9"; };
              "topcoat-mail" = { version = "0.9"; optional = true; };
            };
            features = {
              "default" = [ "router" ];
              "router" = [ "topcoat-mail?/router" ];
            };
          };
        };
        res = features.resolveFeatures {
          root = ./.;
          cargoTomlPath = ./does-not-exist-Cargo.toml;
          lockPackages = [
            { name = "topcoat"; version = "0.9.0"; isWorkspace = true; }
            { name = "topcoat-core"; version = "0.9.0"; }
            { name = "topcoat-mail"; version = "0.9.0"; }
          ];
          readToml = name: mockTomls.${name} or null;
          unifyFeatures = true;
        };
      in
        !(res ? "topcoat-mail") &&
        ((res.__activeDeps."topcoat-0.9.0" or []) == [ "topcoat-core-0.9.0" ]);
    expected = true;
  };

  testResolveFeaturesWithMockToml = {
    expr =
      let
        mockTomls = {
          "h2" = {
            dependencies.tokio-util = {
              version = "0.7";
              features = [ "codec" "io" ];
            };
          };
          "proc-macro-crate" = {
            dependencies.toml_edit = {
              version = "0.25";
              features = [ "parse" ];
            };
          };
        };
        res = features.resolveFeatures {
          root = ./.;
          cargoTomlPath = ./does-not-exist-Cargo.toml;
          lockPackages = [
            { name = "h2"; version = "0.4.19"; }
            { name = "proc-macro-crate"; version = "3.5.0"; }
          ];
          readToml = name: mockTomls.${name} or null;
          unifyFeatures = true;
        };
      in
        (builtins.elem "parse" (res.toml_edit or [])) &&
        (builtins.elem "codec" (res.tokio_util or []));
    expected = true;
  };
}
