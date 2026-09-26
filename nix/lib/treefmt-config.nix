{...}: {
  projectRootFile = "flake.nix";

  programs.deadnix = {
    enable = true;
    priority = 1;
  };
  programs.alejandra = {
    enable = true;
    priority = 2;
  };
  programs.rustfmt = {
    enable = true;
    priority = 1;
  };
  programs.shfmt = {
    enable = true;
    priority = 1;
  };
  programs.shellcheck = {
    enable = true;
    priority = 2;
  };
  programs.taplo = {
    enable = true;
    priority = 2;
  };
  programs.toml-sort = {
    enable = true;
    priority = 1;
  };
  programs.yamlfmt = {
    enable = true;
    priority = 1;
  };
}
