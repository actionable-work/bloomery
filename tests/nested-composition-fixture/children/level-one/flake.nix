{
  outputs = {
    nixpkgs,
    bloomery,
    ...
  }:
    bloomery.mkFlake {
      inherit nixpkgs;
      root = ./.;
    };
}
