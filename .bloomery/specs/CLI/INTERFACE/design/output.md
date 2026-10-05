# Human and machine-readable output

Every application command (`init`, `check`, `review`, `sync`, and `config`)
accepts the same global `--json` flag, before or after the subcommand. No
command has a
separate JSON flag or format selector. It emits one JSON document on stdout, with no ANSI
escapes, progress text, or human-formatted banners mixed into the document.
Usage and operational failures requested with `--json` are also represented as
JSON while retaining their normal exit codes. Parser help remains
human-readable when the flake preflight passes. If the root lacks `flake.nix`,
the shared setup error takes precedence and says to set up a Bloomery
`flake.nix`; `--json` emits one structured setup-error document.

## Human-readable output

Human output is designed for terminals: command headings, statuses, requirement
IDs, source locations, stage progress, and warnings use restrained color to make
important information easy to scan. Color is enabled only for a terminal stream,
when `NO_COLOR` is absent or empty and `TERM` is not `dumb`. Redirected output
is plain text by default. Users can always disable colors by setting a non-empty
`NO_COLOR`.

The check command emits aggregate counts and a bounded failure page, highlighting
status, failure IDs, check IDs, and source locations. Expanded messages, notes,
and logs are retrieved separately. It does not stream build logs or successful
check records by default. Live check metrics use one transient stderr line only
on eligible user terminals, never in CI or JSON mode, and redraw only when metrics
change. Redirecting stdout alone does not suppress terminal stderr progress;
`NO_COLOR` disables its styling, not the updates. See
[check progress](../../CHECK/design/progress.md) and
[check summaries](../../CHECK/design/diagnostics.md). Review retains its indented requirement tree while
highlighting area/feature/group labels, IDs, and the final count. Config prints
command, key, and value lines and marks keys as configured or default. Sync
highlights
stage progress and completion on stdout, and warnings, recommendations, and
failures on stderr. Color never changes the textual content or ordering.

## JSON contracts

- `init --json` emits one object with the command name, status, target
directory, selected template, and created paths. Failure responses carry the
same command identity and an error description.
- `check --json` emits a compact bounded object with run ID, status, outcome
  counts, and a paginated `failures` array. Optional scope identifies subset
  runs. Successful checks are represented by counts, not individual records.
  Failure records contain a short retrieval ID, check ID, code, and optional
  subject/location; paths are repository-relative with `/` separators. Full
  diagnostics and logs live in the retained run, not the default JSON response.
  `check list`, `check failures`, and `check details` emit bounded pages with
  continuation metadata. See the [check schema](../../CHECK/design/diagnostics.md)
  and [retrieval contract](../../CHECK/design/details.md).
- `review --json` emits the existing stable array of manual review items with
  `area`, `feature`, `group`, `id`, `title`, `statement`, and `design_ref`.
- `sync --json` emits an object with `command: "sync"` and `status`. Success
  includes a `result` containing reconciled locks, updated ecosystems, skipped
  updates, completed stages, warnings, recommendations, and tool stderr.
  Failure includes an `error` object with the failed stage, message, completed
  stages, whether locks may be partially synchronized, and recovery guidance
  when needed. Notices remain structured data rather than being printed as
  human-formatted stderr lines.
- `config --json` emits an object with `command: "config"`, the subcommand, and
  `status`. Success includes the addressed key with its effective value,
  configured flag, and recommendation status for `get`, the sorted key records
  for `list`, the affected key and whether the file changed for `set` and
  `unset`, and the sorted added key paths and written flag for `upgrade` or the
  recommendation differences for `upgrade --diff`. An absent `flake.nix` or a
  configuration file that the command created adds a structured warning field.
  Failure carries the same command identity and an
  error description. See the
  [config editing contract](../../CONFIG/design/editing.md#output-and-exit-codes).

JSON is UTF-8, deterministic for the same retained result, and contains no color
codes. The JSON option does not alter the command's exit-code contract.
