{
  description = "Shared Rust workspace source for the benchmark builders";

  outputs = {...}: {
    lib.src = ./.;
  };
}
