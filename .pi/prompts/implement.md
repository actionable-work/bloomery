---
description: Implement the work specified by the root PLAN.md and verify it with Bloomery
---
Implement the specific plan in the root-level `PLAN.md`.

Read `PLAN.md`, the relevant specifications it references, and the repository's `AGENTS.md` before making changes. Treat the plan as the scope: implement its steps in order, respect dependencies, and avoid unrelated changes. Follow the desired behavior in the specifications and the repository's established architecture and conventions. Add or update tests for the planned behavior. Do not rewrite or delete `PLAN.md`; it is the implementation checklist and should remain available for review.

After completing the planned changes, run exactly:

```sh
nix run .#bloomery:dev -- check
```

Work is not done unless that command passes. If it fails, investigate the reported failures, make the necessary in-scope implementation or test fixes, and run the same command again. Repeat until it passes. If you cannot make it pass because of an external blocker or a failure that cannot reasonably be resolved within the plan's scope, do not claim completion; report the exact command result, blocker, and remaining work clearly.

If a planned dependency or crate-layout change is necessary, confirm with the user before adding dependencies and run `bloomery sync` as required by the repository instructions. Do not create commits or push changes. Finish with a concise summary of implemented plan steps, changed paths, and the successful check result (or the blocker preventing it).