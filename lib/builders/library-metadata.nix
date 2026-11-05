{lib}: {pkg}:
lib.optionalString ((pkg.libName or null) == null) ''
  if [ -f Cargo.toml ]; then
    CRATE_NAME=$(awk -v defaultName="$CRATE_NAME" -f ${./library-name.awk} Cargo.toml)
    export CARGO_CRATE_NAME="$CRATE_NAME"
  fi
''
