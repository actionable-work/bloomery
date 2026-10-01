{
  lib,
  workspace,
}:
lib.mapAttrs'
(name: drv: lib.nameValuePair "core:${name}" drv)
workspace.checks
