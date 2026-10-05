{
  pkgs,
  lib ? pkgs.lib,
  treefmtNix ? null,
}: let
  formatters = import ./formatters.nix {inherit pkgs lib treefmtNix;};

  indexOf = order: name: let
    indexed =
      lib.imap0 (index: entry: {
        inherit index;
        formatter = entry;
      })
      order;
  in
    (lib.findFirst (entry: entry.formatter == name) {
        index = -1;
        formatter = "";
      }
      indexed).index;

  previewttier = {
    package = pkgs.hello;
    command = "hello";
    includes = ["*.ts"];
    options = ["--check"];
  };
in
  if treefmtNix == null
  then {}
  else {
    testDefaultFormattersIncludeTomlSort = {
      expr = builtins.elem "toml-sort" formatters.builtinNames && builtins.elem "toml-sort" (formatters.build {}).order;
      expected = true;
    };

    testTomlSortSortsAllKeys = {
      expr = let
        result = formatters.build {};
      in
        builtins.elem "--all" result.treefmt.settings.formatter.toml-sort.options;
      expected = true;
    };

    testTomlSortPreservesRequirementFieldOrder = {
      expr = let
        result = formatters.build {};
      in
        builtins.elem "**/requirements/*.toml" result.treefmt.settings.formatter.toml-sort.excludes;
      expected = true;
    };

    testAbsentFormatterConfigurationEnablesTheDefaultSet = {
      expr = builtins.length (formatters.build {}).order == builtins.length formatters.builtinNames;
      expected = true;
    };

    testFormatterCanBeDisabled = {
      expr = builtins.elem "toml-sort" (formatters.build {config.toml-sort.enable = false;}).order;
      expected = false;
    };

    testOrderingEdgesResolveToTotalOrder = {
      expr = let
        order = (formatters.build {config.rustfmt.before = ["alejandra"];}).order;
      in
        indexOf order "rustfmt" < indexOf order "alejandra";
      expected = true;
    };

    testResolvedOrderIsWiredIntoTreefmt = {
      expr = let
        result = formatters.build {config.rustfmt.before = ["alejandra"];};
      in
        result.treefmt.settings.formatter.rustfmt.priority
        < result.treefmt.settings.formatter.alejandra.priority;
      expected = true;
    };

    testDisabledFormatterEdgesAreIgnored = {
      expr = let
        order =
          (formatters.build {
            config.toml-sort.after = ["taplo"];
            config.taplo.enable = false;
          }).order;
      in
        !(builtins.elem "taplo" order);
      expected = true;
    };

    testOrderingCyclesAreRejected = {
      expr =
        !(builtins.tryEval (
          builtins.length
          (formatters.build {
            config.rustfmt.before = ["alejandra"];
            config.alejandra.before = ["rustfmt"];
          }).order
        )).success;
      expected = true;
    };

    testSelfOrderingIsRejected = {
      expr =
        !(builtins.tryEval (
          builtins.length (formatters.build {config.rustfmt.before = ["rustfmt"];}).order
        )).success;
      expected = true;
    };

    testUnknownFormatterReferencesAreRejected = {
      expr =
        !(builtins.tryEval (
          builtins.length (formatters.build {config.rustfmt.before = ["nope"];}).order
        )).success;
      expected = true;
    };

    testExtraFormattersJoinTheGraph = {
      expr = let
        order =
          (formatters.build {
            extraFormatters.prettier = previewttier;
            config.prettier.before = ["rustfmt"];
          }).order;
      in
        builtins.elem "prettier" order && indexOf order "prettier" < indexOf order "rustfmt";
      expected = true;
    };

    testExtraFormatterBodiesFlowIntoTreefmt = {
      expr = let
        result = formatters.build {extraFormatters.prettier = previewttier;};
      in {
        command = result.treefmt.settings.formatter.prettier.command;
        includes = result.treefmt.settings.formatter.prettier.includes;
        options = result.treefmt.settings.formatter.prettier.options;
        hasPriority = builtins.isInt result.treefmt.settings.formatter.prettier.priority;
      };
      expected = {
        command = "${pkgs.hello}/bin/hello";
        includes = ["*.ts"];
        options = ["--check"];
        hasPriority = true;
      };
    };
  }
