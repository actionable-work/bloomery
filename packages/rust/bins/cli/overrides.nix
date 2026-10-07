{...}: {
  # The default CLI package is intentionally unwrapped: the executable uses the
  # tools available in its surrounding environment. The root flake exports a
  # `bloomery-wrapped` package that wraps the hard runtime tools, currently
  # cargo, for callers that want a self-contained CLI.
}
