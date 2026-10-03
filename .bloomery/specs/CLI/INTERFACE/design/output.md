# Human and machine-readable output

Every application command (`check`, `review`, and `sync`) accepts the same global
`--json` flag, before or after the subcommand. No command has a separate JSON
flag or format selector. It emits one JSON document on stdout, with no ANSI
escapes, progress text, or human-formatted banners mixed into the document.
Usage and operational failures requested with `--json` are also represented as
JSON while retaining their normal exit codes. Parser help remains human-readable.

## Human-readable output

Human output is designed for terminals: command headings, statuses, requirement
IDs, source locations, stage progress, and warnings use restrained color to make
important information easy to scan. Color is enabled only for a terminal stream,
when `NO_COLOR` is absent or empty and `TERM` is not `dumb`. Redirected output
is plain text by default. Users can always disable colors by setting a non-empty
`NO_COLOR`.

The check command highlights pass/fail status, diagnostic severity, source
locations, and notes. Review retains its indented requirement tree while
highlighting area/feature/group labels, IDs, and the final count. Sync highlights
stage progress and completion on stdout, and warnings, recommendations, and
failures on stderr. Color never changes the textual content or ordering.

## JSON contracts

- `check --json` emits an object with `command: "check"`, `status` (`"passed"`
  or `"failed"`), and a `diagnostics` array. Each diagnostic has `code`,
  `message`, nullable `location` (`path`, nullable `line` and `column`), and
  `notes`. Paths are repository-relative and use `/` separators. A successful
  check has an empty diagnostics array.
- `review --json` emits the existing stable array of manual review items with
  `area`, `feature`, `group`, `id`, `title`, `statement`, and `design_ref`.
- `sync --json` emits an object with `command: "sync"` and `status`. Success
  includes a `result` containing reconciled locks, updated ecosystems, skipped
  updates, completed stages, warnings, recommendations, and tool stderr.
  Failure includes an `error` object with the failed stage, message, completed
  stages, whether locks may be partially synchronized, and recovery guidance
  when needed. Notices remain structured data rather than being printed as
  human-formatted stderr lines.

JSON is UTF-8, deterministic for the same command result, and contains no color
codes. The JSON option does not alter the command's exit-code contract.
