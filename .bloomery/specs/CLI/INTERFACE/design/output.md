# Human and machine-readable output

Every application command (`init`, `check`, `build`, `disk`, `review`, `sync`,
`spec`, and `config`) accepts the same global `--json` flag, before or after the
subcommand. No command has a separate JSON flag or format selector. Human output
is the default; `--json` selects the machine contract. Usage and operational
failures requested with `--json` are represented as JSON while retaining their
normal exit codes.

## Human-readable output

Human output follows the shared [presentation grammar](presentation.md): one root
heading, the shared tree grammar for hierarchy, the shared status vocabulary and
semantic color palette, notices on stderr, and one closing summary line.
Command-specific designs describe only their content, not a separate visual
style.

Human output is designed for terminals: command headings, statuses, requirement
IDs, source locations, stage progress, and warnings use the shared color roles to
make important information easy to scan. Color is enabled only for an eligible
stream: a terminal stream, with an absent or empty `NO_COLOR`, a `TERM` that is
not `dumb`, and human mode selected. Redirected output is plain by default.
`--color=always` forces styling of human output and `--color=never` disables it;
`--json` is always escape-free.

The check command emits aggregate counts and a bounded failure page, highlighting
status, failure IDs, check IDs, and source locations. Expanded messages, notes,
and logs are retrieved separately. It does not stream build logs or successful
check records by default. The build command prints each realized package with
its output store path and a pass/fail summary; it likewise does not stream raw
build logs. Live check metrics use one transient stderr line only on eligible
user terminals, never in CI or JSON mode, and redraw only when metrics change.
Redirecting stdout alone does not suppress terminal stderr progress; `NO_COLOR`
disables its styling, not the updates. See
[check progress](../../CHECK/design/progress.md) and
[check summaries](../../CHECK/design/diagnostics.md). Review renders its
indented requirement tree with highlighted area/feature/group labels, IDs, and
the final count. Config prints command, key, and value lines and marks keys as
configured or default. Sync highlights stage progress and completion on stdout,
and warnings, recommendations, and failures on stderr. Styling never changes the
textual content or ordering.

## JSON contracts

### JSON envelope

Every command emits exactly one JSON document on stdout under a shared envelope:

```json
{
  "command": "review",
  "status": "succeeded",
  "result": {},
  "warnings": []
}
```

- `command` is the stable command name: `init`, `check`, `build`, `disk`,
  `review`, `sync`, `spec`, or `config`.
- `operation` names the nested subcommand when one was selected and is omitted
  otherwise.
- `status` is the command outcome: `succeeded`, `failed`, `error`, or
  `interrupted`.
- `result` carries the command-specific payload and is omitted when the command
  fails.
- `warnings` is an array of `{ "code", "message" }` records and is omitted when
  empty.
- A failure replaces `result` with an `error` object carrying a stable `code`, a
  `kind`, and a `message`, plus optional structured details.

```json
{"command":"build","status":"error","error":{"kind":"operational","code":"MissingNix","message":"nix was not found on PATH"}}
```

Command-specific payloads are carried under `result`; the sections below describe
them.

### Result payloads

- `init` carries the target `directory`, the selected `template`, the `created`
  paths, and the generated `lockfiles`.
- `check` carries the run ID, run `status`, outcome `counts`, and a paginated
  `failures` array. Optional `scope` identifies subset runs. Successful checks
  are represented by counts, not individual records. Failure records contain a
  short retrieval ID, check ID, code, and optional subject/location; paths are
  repository-relative with `/` separators. Full diagnostics and logs live in the
  retained run, not the default JSON response. `check list`, `check failures`,
  and `check details` carry bounded pages with continuation metadata. See the
  [check schema](../../CHECK/design/diagnostics.md) and
  [retrieval contract](../../CHECK/design/details.md).
- `build` carries the selected `systems` and a `packages` array whose entries
  carry name, system, status, and realized store path, plus a derivation path
  when the package failed. When `--check` is selected it also carries the gate
  status and, on failure, the failing check IDs. See the
  [build output contract](../../BUILD/design/execution.md#output).
- `review` carries the stable array of manual review items with `area`,
  `feature`, `group`, `id`, `title`, `statement`, and `design_ref`.
- `disk` carries the measured total as an integer `bytes` field, plus the
  `categories` tree where each category has a `name` and `entries`, and each
  entry has a `name`, `bytes`, and `unmeasured` flag. A full-tree measurement
  includes the measured `systems`; an addressed measurement includes the
  `derivation`. Unmeasured derivations are listed with their addresses. See the
  [measurement contract](../../DISK/design/measurement.md#output) and
  [category report](../../DISK/design/report.md).
- `sync` success carries reconciled locks, updated ecosystems, skipped updates,
  completed stages, warnings, recommendations, and tool stderr. `sync` failure
  carries the failed stage, message, completed stages, whether locks may be
  partially synchronized, and recovery guidance when needed. Notices remain
  structured data rather than human-formatted stderr lines.
- `config` carries the addressed key with its effective value, configured flag,
  and recommendation status for `get`, the sorted key records for `list`, the
  affected key and whether the file changed for `set` and `unset`, and the
  sorted added key paths and written flag for `upgrade` or the recommendation
  differences for `upgrade --diff`. Documented key paths are carried by
  `document`.
- `spec` read results carry the sorted record list for `list`, or the addressed
  record with its fields and canonical EARS statement for `show`. Mutations
  report the affected ID and fields; `remove` reports the removed ID and whether
  its group file was pruned. `spec trace` carries one record per requested
  requirement with its tied test references, each carrying the scanner kind,
  repository-relative path, and line. `spec candidates` carries an array of
  untied test sites with the scanner kind, test name when available, path, and
  line.

### Machine guarantees

- In `--json` mode stdout carries exactly the one envelope document; stderr
  carries no human-formatted notices or progress. Warnings, diagnostics, and
  surfaced tool output are fields of the document.
- JSON documents contain no ANSI escapes, cursor control, or transient
  progress, and are produced identically whether or not stdout is a terminal.
- JSON is UTF-8, deterministic for the same retained result, and stable in field
  names and ordering.
- The JSON option does not alter the command's exit-code contract.
