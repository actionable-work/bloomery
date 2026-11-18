# Billet outputs

The optional `mkFlake.billets` callback receives `{ pkgs, packages }` once for
each configured system. `packages` is the workspace and elevated sub-flake
package set, before any `extraOutputs` changes. The callback returns a named set
of evaluated services and bundles. Each member supplies a Nix derivation as
`validationCheck`.

Bloomery exports the named set unchanged at `billets.<system>` and adds
`checks.<system>."billet:<name>:validate"` for each member. Other checks remain
available. Sub-flakes contribute Billets through
[composition](composition.md#elevated-billets).

`billets` must be callable. A malformed result names the owning system in its
diagnostic; a member without a validation derivation names the system and
member. Generated check names may not collide with existing checks. Callback or
elevated Billets exclude an `extraOutputs.billets` definition. Without either,
`billets` stays an ordinary `extraOutputs` family.

The application supplies any Billet flake input and picks the targets and
packages. Bloomery imports no Billet source.
