{pkgs}:
pkgs.runCommand "bloomery-topcoat-bundler-failures" {
  nativeBuildInputs = [pkgs.python3];
  passthru.bloomery = [
    "NIXLIB-GRAPH-ASSETS-005"
    "NIXLIB-GRAPH-ASSETS-007"
    "NIXLIB-GRAPH-ASSETS-010"
    "NIXLIB-GRAPH-ASSETS-018"
  ];
} ''
  python3 ${./topcoat-bundler-test.py} ${../../lib/builders/topcoat-bundler.py} failures
  mkdir "$out"
  echo "OK" > "$out/success"
''
