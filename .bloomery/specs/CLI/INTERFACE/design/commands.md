# Commands and invocation

The `bloomery` executable exposes the application commands and the CLI parser's
built-in help command:

```text
bloomery check
bloomery review
bloomery help [COMMAND]
```

The parser also handles help requests such as `bloomery --help` and
`bloomery check --help`. The `check` and `review` commands operate on the
repository rooted at the process's current working directory; the help command
does not load a repository. Command semantics are specified by
[CHECK](../../CHECK/README.md) and [REVIEW](../../REVIEW/README.md).
