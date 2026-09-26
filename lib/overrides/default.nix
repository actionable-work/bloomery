{ pkgs, lib }:
let
  darwinFrameworks = lib.optionals pkgs.stdenv.hostPlatform.isDarwin (
    with pkgs.darwin.apple_sdk.frameworks; [
      Security
      SystemConfiguration
      CoreFoundation
    ]
  );
in {
  openssl-sys = {
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = [ pkgs.openssl ];
  };

  libpq-sys = {
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = [ pkgs.libpq ];
  };

  pq-sys = {
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = [ pkgs.libpq ];
  };

  libz-sys = {
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = [ pkgs.zlib ];
  };

  zstd-sys = {
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = [ pkgs.zstd ];
  };

  sqlite3-sys = {
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = [ pkgs.sqlite ];
  };

  ring = {
    nativeBuildInputs = [ pkgs.perl ];
  };

  curl-sys = {
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = [ pkgs.curl pkgs.openssl ];
  };

  security-framework-sys = {
    buildInputs = darwinFrameworks;
  };
}
