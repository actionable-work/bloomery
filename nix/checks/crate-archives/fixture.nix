# A dependency diamond with a proc macro shared by the crate archive checks.
{
  pkgs,
  lib,
}: rec {
  builders = import ../../../lib/builders/default.nix {inherit pkgs lib;};
  buildCrate = builders.buildCrateWith {};
  staging = import ../../../lib/builders/dependencies.nix {inherit lib;} {};

  # d -> {b, c}, b -> {a, m}, c -> a.
  crate = name: text: args:
    buildCrate ({
        pkg = {
          inherit name;
          version = "0.1.0";
          crateName = name;
          isWorkspace = true;
        };
        src = pkgs.writeTextDir "src/lib.rs" text;
        edition = "2021";
      }
      // args);
  a = crate "a" "pub fn a() -> u32 { 1 }" {};
  m =
    crate "m" ''
      use proc_macro::TokenStream;
      #[proc_macro]
      pub fn answer(_: TokenStream) -> TokenStream { "42".parse().unwrap() }
    '' {
      isProcMacro = true;
    };
  b = crate "b" "pub fn b() -> u32 { a::a() + m::answer!() }" {dependencies = [a m];};
  c = crate "c" "pub fn c() -> u32 { a::a() * 2 }" {dependencies = [a];};
  dSource = "pub fn d() -> u32 { b::b() + c::c() }";
  d = crate "d" dSource {dependencies = [b c];};
  # The same crate as an independent derivation.
  dRebuilt = crate "d" dSource {
    dependencies = [b c];
    override.env.BLOOMERY_REBUILD_SALT = "independent";
  };
  main = pkgs.writeText "main.rs" ''
    fn main() { println!("{}", d::d()); }
  '';

  check = name: attrs: script:
    pkgs.runCommand "bloomery-crate-archives-${name}" ({
        nativeBuildInputs = [pkgs.rustc pkgs.stdenv.cc pkgs.lld pkgs.zstd pkgs.diffutils];
      }
      // attrs) ''
      set -euo pipefail
      ${staging}
      fail() { echo "$*" >&2; exit 1; }
      ${script}
      mkdir "$out"
      echo OK > "$out/success"
    '';
}
