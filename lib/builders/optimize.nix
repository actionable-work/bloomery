{
  pkgs,
  lib,
  rustc ? pkgs.rustc,
  stdenv ? pkgs.stdenv,
  mold ? pkgs.mold,
  lld ? pkgs.lld,
  useMold ? null,
  useLld ? null,
  defaultLinker ? (
    if useLld != null
    then
      (
        if useLld
        then "lld"
        else null
      )
    else if useMold != null
    then
      (
        if useMold
        then "mold"
        else null
      )
    else if stdenv.hostPlatform.isLinux
    then "lld"
    else null
  ),
}: rec {
  # Resolve the LLVM profiling and BOLT tools that match the selected rustc.
  # A toolchain without `passthru.llvmPackages`, or a set that lacks the `llvm`
  # or `bolt` members, fails evaluation instead of silently skipping
  # optimization.
  resolveTools = {rustc ? pkgs.rustc}: let
    llvmPackages =
      if rustc ? passthru && rustc.passthru ? llvmPackages
      then rustc.passthru.llvmPackages
      else pkgs.llvmPackages or null;
    llvm =
      if llvmPackages != null && llvmPackages ? llvm
      then llvmPackages.llvm
      else null;
    bolt =
      if llvmPackages != null && llvmPackages ? bolt
      then llvmPackages.bolt
      else null;
    profdata = "${llvm}/bin/llvm-profdata";
    boltBinary = "${bolt}/bin/llvm-bolt";
    fdataMerger = "${bolt}/bin/merge-fdata";
  in
    if llvm == null || !(builtins.pathExists profdata)
    then throw "bloomery: the selected rustc toolchain does not provide the LLVM profiling tools (llvm-profdata) required for optimization"
    else if bolt == null || !(builtins.pathExists boltBinary) || !(builtins.pathExists fdataMerger)
    then throw "bloomery: the selected rustc toolchain does not provide the BOLT tools (llvm-bolt, merge-fdata) required for optimization"
    else {
      inherit llvm bolt;
      inherit profdata;
      llvmBolt = boltBinary;
      mergeFdata = fdataMerger;
    };

  # Materialize a crate-level training fixture input. Path values are copied
  # whole; fileset values are selected against the crate root.
  materializeFixtures = {
    crateDir,
    fileset ? null,
  }:
    if fileset == null
    then null
    else if builtins.isPath fileset || builtins.isString fileset
    then
      builtins.path {
        name = "optimize-fixtures";
        path = fileset;
      }
    else
      lib.fileset.toSource {
        root = crateDir;
        fileset = fileset;
      };

  # Build the complete PGO then BOLT pipeline for one binary. Returns the final
  # install derivation plus every intermediate stage so tests can inspect the
  # pipeline graph.
  build = {
    binName,
    pkg,
    src,
    entry ? null,
    crateDrv ? null,
    dependencies ? [],
    # Profile variants of the workspace dependency closure. Defaults keep the
    # release closure, which is what direct builder tests use.
    instrumentedCrateDrv ? crateDrv,
    instrumentedDependencies ? dependencies,
    mkProfileUseDeps ? (_merged: {inherit crateDrv dependencies;}),
    override ? {},
    profile ? {},
    defaultRustcFlags ? ["-Copt-level=3"],
    edition ? null,
    workspaceAssets ? null,
    workspaceStatic ? null,
    workspacePublic ? null,
    # Training inputs.
    root,
    script,
    crateDir ? null,
    fixtureFileset ? null,
    trainingNativeBuildInputs ? [],
    trainingBuildInputs ? [],
    trainingEnv ? {},
    # Stage selections. Defaults run the complete pipeline.
    pgo ? {},
    bolt ? {},
  }: let
    builderBin = import ./bin.nix {
      inherit
        pkgs
        lib
        rustc
        stdenv
        mold
        lld
        useMold
        useLld
        defaultLinker
        ;
    };

    tools = resolveTools {inherit rustc;};

    scriptSource =
      if script == null
      then null
      else
        lib.fileset.toSource {
          root = root;
          fileset = script;
        };
    scriptRel =
      if script == null
      then null
      else lib.removePrefix (toString root + "/") (toString script);

    fixtures = materializeFixtures {
      crateDir =
        if crateDir != null
        then crateDir
        else root;
      fileset = fixtureFileset;
    };

    # The training script and additive fixtures are isolated inputs of the
    # training derivations only. They are never part of the compilation source.
    trainingSources =
      if scriptSource == null
      then null
      else
        pkgs.runCommand "optimize-training-sources" {} (
          ''
            mkdir -p "$out"
            cp -r ${scriptSource}/. "$out/"
          ''
          + lib.optionalString (fixtures != null) ''
            cp -r ${fixtures}/. "$out/"
          ''
        );

    runtimeDependencies = override.runtimeDependencies or [];
    trainingPathInputs =
      trainingNativeBuildInputs ++ trainingBuildInputs ++ runtimeDependencies;

    mkTraining = {
      name,
      instrumentedBinary,
      boltMode,
    }:
      pkgs.stdenv.mkDerivation {
        name = "${binName}-${name}";
        nativeBuildInputs = trainingNativeBuildInputs;
        buildInputs = trainingBuildInputs;
        BLOOMERY_TRAIN_BINARY = "${instrumentedBinary}/bin/${binName}";

        buildCommand = ''
          runHook preBuild
          cp -r ${trainingSources}/. .
          chmod -R u+w .

          export BLOOMERY_TRAIN_BINARY="$BLOOMERY_TRAIN_BINARY"
          export BLOOMERY_PROFILE_DIR="$PWD/profile"
          mkdir -p "$BLOOMERY_PROFILE_DIR"
          export PATH="${instrumentedBinary}/bin:${lib.makeBinPath trainingPathInputs}:$PATH"
          ${
            if boltMode
            then ""
            else ''export LLVM_PROFILE_FILE="$BLOOMERY_PROFILE_DIR/default_%m.profraw"''
          }
          ${lib.concatStringsSep "\n" (
            lib.mapAttrsToList (key: value: ''export ${key}=${lib.escapeShellArg value}'')
            trainingEnv
          )}

          bash "${scriptRel}"

          runHook postBuild
          runHook preInstall
          mkdir -p "$out"
          cp -r "$BLOOMERY_PROFILE_DIR"/. "$out/"
          runHook postInstall
        '';
      };

    pgoSelection = {
      enable = pgo.enable or true;
      scope = pgo.scope or "workspace";
    };
    boltSelection = {
      enable = bolt.enable or true;
      functions = bolt.functions or true;
      blocks = bolt.blocks or true;
    };
    pgoEnabled = pgoSelection.enable;
    boltEnabled = boltSelection.enable;
    boltFunctions = boltSelection.functions;
    boltBlocks = boltSelection.blocks;
    # Function reordering requires relocations and keeps the original sections,
    # which grows the binary; block-only does not.
    relocations = boltEnabled && boltFunctions;
    relocationFlags = lib.optionals relocations [
      "-Clink-arg=-Wl,--emit-relocs"
      "-Cstrip=none"
    ];
    pgoUseFlags = ["-Cllvm-args=-pgo-warn-missing-function"];
    boltFlags =
      lib.optionals boltBlocks ["--reorder-blocks=ext-tsp"]
      ++ lib.optionals boltFunctions ["--reorder-functions=hfsort"];

    # PGO: compile with profile generation, train, merge, then compile with
    # profile use. The whole stage is skipped when disabled.
    instrumented =
      if !pgoEnabled
      then null
      else
        builderBin {
          inherit
            binName
            pkg
            src
            entry
            edition
            workspaceAssets
            workspaceStatic
            workspacePublic
            profile
            defaultRustcFlags
            ;
          crateDrv = instrumentedCrateDrv;
          dependencies = instrumentedDependencies;
          override =
            override
            // {
              rustcFlags = (override.rustcFlags or []) ++ ["-Cprofile-generate"];
            };
        };

    pgoTraining =
      if !pgoEnabled
      then null
      else
        mkTraining {
          name = "pgo-training";
          instrumentedBinary = instrumented;
          boltMode = false;
        };

    pgoMerged =
      if !pgoEnabled
      then null
      else
        pkgs.runCommand "${binName}-pgo-merged" {
          nativeBuildInputs = [tools.llvm];
        } ''
          ${tools.profdata} merge -o "$out" ${pgoTraining}/*.profraw
        '';

    profileUse =
      if pgoEnabled
      then mkProfileUseDeps pgoMerged
      else null;

    pgoCompiled =
      if !pgoEnabled
      then null
      else
        builderBin {
          inherit
            binName
            pkg
            src
            entry
            edition
            workspaceAssets
            workspaceStatic
            workspacePublic
            profile
            defaultRustcFlags
            ;
          crateDrv = profileUse.crateDrv;
          dependencies = profileUse.dependencies;
          override =
            override
            // {
              rustcFlags =
                (override.rustcFlags or [])
                ++ ["-Cprofile-use=${pgoMerged}"]
                ++ pgoUseFlags
                ++ relocationFlags;
            };
        };

    # BOLT operates on the PGO result when PGO is enabled, otherwise on a
    # release build that preserves relocations when function reordering needs
    # them.
    boltInput =
      if pgoEnabled
      then pgoCompiled
      else
        builderBin {
          inherit
            binName
            pkg
            src
            entry
            crateDrv
            dependencies
            edition
            workspaceAssets
            workspaceStatic
            workspacePublic
            profile
            defaultRustcFlags
            ;
          override =
            override
            // {
              rustcFlags = (override.rustcFlags or []) ++ relocationFlags;
            };
        };

    boltInstrumented =
      if !boltEnabled
      then null
      else
        pkgs.runCommand "${binName}-bolt-instrumented" {
          nativeBuildInputs = [tools.bolt];
        } ''
          mkdir -p "$out/bin"
          ${tools.llvmBolt} ${boltInput}/bin/${binName} \
            -instrument \
            --instrumentation-file=profile/bolt.fdata \
            -o "$out/bin/${binName}"
          chmod +x "$out/bin/${binName}"
        '';

    boltTraining =
      if !boltEnabled
      then null
      else
        mkTraining {
          name = "bolt-training";
          instrumentedBinary = boltInstrumented;
          boltMode = true;
        };

    boltMerged =
      if !boltEnabled
      then null
      else
        pkgs.runCommand "${binName}-bolt-merged" {
          nativeBuildInputs = [tools.bolt];
        } ''
          ${tools.mergeFdata} ${boltTraining}/*.fdata > "$out"
        '';

    boltOptimized =
      if !boltEnabled
      then null
      else
        pkgs.runCommand "${binName}-bolt-optimized" {
          nativeBuildInputs = [tools.bolt];
        } ''
          mkdir -p "$out/bin"
          ${tools.llvmBolt} ${boltInput}/bin/${binName} \
            -data ${boltMerged} \
            -o "$out/bin/${binName}" \
            ${lib.escapeShellArgs boltFlags}
          chmod +x "$out/bin/${binName}"
        '';

    optimizedBinary =
      if boltEnabled
      then boltOptimized
      else boltInput;

    # Reuse the release install phases (assets and runtime wrapping) for the
    # optimized executable.
    package = builderBin {
      inherit
        binName
        pkg
        src
        entry
        crateDrv
        dependencies
        edition
        workspaceAssets
        workspaceStatic
        workspacePublic
        override
        profile
        defaultRustcFlags
        ;
      prebuilt = optimizedBinary;
    };
  in {
    inherit
      package
      instrumented
      pgoTraining
      pgoMerged
      pgoCompiled
      boltInput
      boltInstrumented
      boltTraining
      boltMerged
      boltOptimized
      trainingSources
      pgoEnabled
      boltEnabled
      ;
  };
}
