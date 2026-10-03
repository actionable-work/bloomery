---
description: Remove the implementation plan, commit the changeset with jj, and return PR recommendations
---
Clean up and commit the current changeset, then provide only a proposed pull-request title and description.

First remove the root-level `PLAN.md` if it exists. Then inspect the remaining working-copy changes with `jj status`, `jj diff -s`, and the full `jj diff`. Do not discard or amend unrelated changes; the commit must reflect the remaining current changeset. If there are no remaining changes to commit, do not create an empty commit.

Run the repository's required verification command before committing:

```sh
nix run .#bloomery:dev -- check
```

If verification fails, do not commit. Do not resolve failures, instead immediately stop and provide the error details to the user.

Inspect recent commit descriptions and create a concise conventional commit message matching repository style. In this jujutsu repository, describe the current working-copy change using `jj describe -m`, then advance to a fresh working copy with `jj new`. Do not use `git commit` and do not push. Confirm the resulting commit and clean working copy with `jj status` and `jj log`.

Derive a potential PR title and description from the actual committed changes. The PR title must use Conventional Commit format (for example, `feat: ...`, `fix: ...`, or `docs: ...`). The description should briefly summarize the purpose and key changes and mention verification. Do not claim changes or checks that did not occur.

Your final response must contain only these two recommendations, with no preamble, status commentary, or other text:

# PR title:
<conventional-commit title>

# PR description:
<concise proposed description>