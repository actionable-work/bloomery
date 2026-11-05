{
  pkgs,
  lib,
}: let
  cases = [
    {
      text = ''        lib.name = """
        actual_library"""'';
      expected = "actual_library";
    }
    {
      text = ''        lib.name = """actual_\
                 library"""'';
      expected = "actual_library";
    }
    {
      text = ''        [lib]
        name = "actual_library"
      '';
      expected = "actual_library";
    }
    {
      text = ''        [ 'lib' ] # target
        'name' = 'actual_library'
      '';
      expected = "actual_library";
    }
    {
      text = ''lib.name = "actual_library"'';
      expected = "actual_library";
    }
    {
      text = ''lib = { path = "src/custom.rs", name = "actual_library" }'';
      expected = "actual_library";
    }
    {
      text = ''lib.name = "actual_\u006cibrary"'';
      expected = "actual_library";
    }
    {
      text = ''        [package]
        name = "package-name"
        description = """
        [lib]
        name = "not_a_target"
        """
        # [lib]
      '';
      expected = "package_name";
    }
  ];
in
  pkgs.runCommand "bloomery-library-name-reader" {} ''
    ${lib.concatMapStringsSep "\n" (sample: let
      manifest = builtins.toFile "Cargo.toml" sample.text;
    in ''test "$(awk -v defaultName=package_name -f ${../../lib/builders/library-name.awk} ${manifest})" = ${lib.escapeShellArg sample.expected}'')
    cases}
    mkdir "$out"
  ''
