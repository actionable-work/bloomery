# Native Tailwind CLI provisioning for Topcoat's build-script integration.
#
# A crate derivation provides the integration when it is `topcoat-tailwind`
# compiled with its `build` feature, or when any of its dependencies provides
# it. A build script whose dependencies provide it receives the nixpkgs
# Tailwind CLI on PATH and through TAILWIND_CLI.
{
  pkgs,
  lib,
}: let
  cli = pkgs.buildPackages.tailwindcss_4;
  providedBy = dependency: dependency.bloomeryProvidesTailwind or false;
  provision = needed: {
    inherit needed;
    nativeBuildInputs = lib.optional needed cli;
    env = lib.optionalAttrs needed {TAILWIND_CLI = lib.getExe' cli "tailwindcss";};
  };
in {
  inherit cli;

  provides = {
    pkg,
    features,
    dependencies,
  }: let
    active =
      if features == null
      then ["default"]
      else features;
  in
    (pkg.name == "topcoat-tailwind" && (lib.elem "build" active || lib.elem "default" active))
    || lib.any providedBy dependencies;

  # Provisioning for a build script with the given dependency derivations.
  forBuildScript = buildScriptDependencies: provision (lib.any providedBy buildScriptDependencies);

  # Provisioning for a development shell covering the given crate derivations.
  forShell = crates: provision (lib.any (crate: crate.bloomeryNeedsTailwind or false) crates);
}
