# Requirement identifiers

Every requirement identifier has four hyphen-separated segments:

```text
<AREA>-<FEATURE>-<GROUP>-<SEQUENCE>
```

The intended grammar is:

```regex
^[A-Z0-9_]+-[A-Z0-9_]+-[A-Z0-9_]+-[0-9]{3,4}$
```

| Segment | Source of truth | Physical match |
| --- | --- | --- |
| `AREA` | Area directory and README `id` | `.bloomery/specs/<AREA>/` |
| `FEATURE` | Feature directory and README `id` | `.../<FEATURE>/` |
| `GROUP` | Requirement file stem and TOML `group` | `.../requirements/<GROUP>.toml` |
| `SEQUENCE` | Numeric record sequence | Unique within the feature group |

The checker compares each segment exactly. It also requires the complete ID to
be globally unique and the sequence to be unique within its group. The
filesystem is therefore an executable index of requirement identity rather
than a presentation-only hierarchy.

The sequence uses three or four digits to preserve readable ordering as groups
grow. Sorting is numeric for CLI output, while the stored identifier preserves
its original zero padding.
