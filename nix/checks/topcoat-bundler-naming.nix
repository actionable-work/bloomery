{pkgs}:
pkgs.runCommand "bloomery-topcoat-bundler-naming" {
  nativeBuildInputs = [pkgs.python3];
  passthru.bloomery = [
    "NIXLIB-GRAPH-ASSETS-008"
    "NIXLIB-GRAPH-ASSETS-009"
  ];
} ''
  python3 ${./topcoat-bundler-test.py} ${../../lib/builders/topcoat-bundler.py} naming
  mkdir "$out"
  echo "OK" > "$out/success"
''
