# Resolve a fixture crate whose name is unique in its Cargo.lock.
lock: name: let
  id = lock.resolveDepId {
    inherit name;
    version = null;
    raw = name;
  };
in
  lock.byId.${id}
