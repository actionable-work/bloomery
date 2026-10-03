---
description: Plan implementation and test work for changed specifications
argument-hint: "[optional focus]"
---
Create a concrete implementation plan for bringing the repository from its current state to the desired state described by the specifications changed in the current changeset. Optional user focus:

${ARGUMENTS:-No additional focus.}

Before any other action, check whether `PLAN.md` exists at the repository root. If it exists, stop immediately and report an error; do not read, modify, or overwrite it. Do not remove or rename an existing file. Only proceed when the path does not exist.

Inspect the current changeset using the repository's jujutsu working-copy diff. Identify specification files under `.bloomery/specs/` that are part of that changeset, and read their linked feature overviews, designs, requirements, and any relevant neighboring architecture. Do not treat unrelated unchanged specifications as the plan's scope. If there are no specification changes in the current changeset, report that no plan can be generated from changed specifications and do not create `PLAN.md`.

For each changed requirement or design behavior, determine whether corresponding implementation and automated tests exist in the current repository. Use available evidence in source and test files and the repository's requirement-evidence conventions; run read-only inspection and validation commands such as `bloomery check` when useful. Distinguish clearly between missing implementation, missing automated test coverage, and behavior already covered. Do not infer coverage from a requirement merely being documented. Include the relevant specification requirement IDs and paths, and point to existing or proposed implementation/test paths where possible.

Write a prioritized, concrete plan to the root-level `PLAN.md`. Organize it into ordered steps with specific outcomes, files or components to change, tests to add or update, and validation commands/acceptance criteria. Address the gaps between current implementation and each changed specification's desired state, including tests for requirements without corresponding tests. Respect dependencies between steps. The plan is for future work: do not implement it, edit specifications, add tests, change source, or modify any other file. The only file you may create or edit is the previously confirmed-absent root `PLAN.md`.

Keep the plan concise but actionable. Call out assumptions, blockers, and uncertain coverage explicitly rather than presenting guesses as facts. After writing `PLAN.md`, summarize its scope and path; do not make any further file changes.