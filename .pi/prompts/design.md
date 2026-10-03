---
description: Create Bloomery design and requirements for a user-requested change without implementing it
argument-hint: "<design request>"
---
Create a design and specification for this request:

$ARGUMENTS

This is a design-only workflow. Do not implement the request, modify source code, tests, configuration, documentation outside the specifications, dependencies, or lockfiles. You may inspect the repository to understand its architecture and conventions, but the only files you may create, edit, or delete are inside `.bloomery/specs/`. Do not modify `.bloomery/config.toml` or any file outside `.bloomery/specs/`.

Follow the repository's existing Bloomery specification structure and formats. Identify the appropriate area and feature; update its overview and design documents, and add or update focused requirements that describe the desired state. Requirements should each express one testable story, should be automated where possible, and should map to relevant design documents. Keep design prose terse and describe the desired state, not implementation gaps or a migration path. Do not claim implementation evidence or make code changes.

First inspect the existing relevant specifications and repository architecture. Then create or update only the necessary files under `.bloomery/specs/`. Finish with a concise summary listing changed specification paths and any important assumptions or open questions. If the request is ambiguous in a way that prevents a coherent specification, ask focused questions before editing.