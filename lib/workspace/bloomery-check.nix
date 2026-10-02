{pkgs}: {
  root,
  bloomeryPackage,
}:
pkgs.runCommand "bloomery-check" {
  nativeBuildInputs = [bloomeryPackage];
} ''
  cd ${root}
  bloomery check
  mkdir -p "$out"
''
