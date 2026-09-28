{
  pkgs,
  lib,
}: let
  darwinFrameworks = lib.optionals pkgs.stdenv.hostPlatform.isDarwin (
    with pkgs.darwin.apple_sdk.frameworks; [
      Security
      SystemConfiguration
      CoreFoundation
    ]
  );
in {
  openssl-sys = {
    nativeBuildInputs = [pkgs.pkg-config];
    buildInputs = [pkgs.openssl];
  };

  libpq-sys = {
    nativeBuildInputs = [pkgs.pkg-config];
    buildInputs = [pkgs.libpq];
  };

  pq-sys = {
    nativeBuildInputs = [pkgs.pkg-config];
    buildInputs = [pkgs.libpq];
  };

  libz-sys = {
    nativeBuildInputs = [pkgs.pkg-config];
    buildInputs = [pkgs.zlib];
  };

  zstd-sys = {
    nativeBuildInputs = [pkgs.pkg-config];
    buildInputs = [pkgs.zstd];
  };

  sqlite3-sys = {
    nativeBuildInputs = [pkgs.pkg-config];
    buildInputs = [pkgs.sqlite];
  };

  ring = {
    nativeBuildInputs = [pkgs.perl];
  };

  curl-sys = {
    nativeBuildInputs = [pkgs.pkg-config];
    buildInputs = [pkgs.curl pkgs.openssl];
  };

  security-framework-sys = {
    buildInputs = darwinFrameworks;
  };

  core-foundation-sys = {
    buildInputs = darwinFrameworks;
  };

  core-graphics-types = {
    buildInputs = darwinFrameworks;
  };

  io-kit-sys = {
    buildInputs = lib.optionals pkgs.stdenv.hostPlatform.isDarwin (
      with pkgs.darwin.apple_sdk.frameworks; [
        IOKit
        CoreFoundation
      ]
    );
  };

  aws-lc-sys = {
    nativeBuildInputs = [pkgs.cmake] ++ lib.optionals pkgs.stdenv.hostPlatform.isDarwin [pkgs.darwin.cctools];
  };

  bzip2-sys = {
    nativeBuildInputs = [pkgs.pkg-config];
    buildInputs = [pkgs.bzip2];
  };

  lz4-sys = {
    nativeBuildInputs = [pkgs.pkg-config];
    buildInputs = [pkgs.lz4];
  };

  libgit2-sys = {
    nativeBuildInputs = [pkgs.pkg-config];
    buildInputs = [pkgs.libgit2 pkgs.openssl pkgs.zlib];
  };

  libssh2-sys = {
    nativeBuildInputs = [pkgs.pkg-config];
    buildInputs = [pkgs.libssh2 pkgs.openssl pkgs.zlib];
  };

  libudev-sys = {
    nativeBuildInputs = [pkgs.pkg-config];
    buildInputs = lib.optionals pkgs.stdenv.hostPlatform.isLinux [pkgs.systemdLibs];
  };

  prost-build = {
    nativeBuildInputs = [pkgs.protobuf];
    env = {
      PROTOC = "${pkgs.protobuf}/bin/protoc";
    };
  };
}
