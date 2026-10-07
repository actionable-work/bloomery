{pkgs, ...}: {
  # `cargo` is the only hard runtime tool the CLI invokes. `nix` is assumed to
  # be provided by the host and `git` is best-effort metadata, so neither is
  # wrapped.
  runtimeDependencies = [pkgs.cargo];
}
