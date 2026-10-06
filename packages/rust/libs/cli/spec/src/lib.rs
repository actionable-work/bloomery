#![allow(clippy::result_large_err)]

mod editing;
mod trace;

use bloomery_cli_types::SpecOperation;
use bloomery_model::{Context, Diagnostic, Requirement};
use serde::Serialize;
use std::path::Path;

/// Classification of a spec command failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecErrorKind {
    /// Invalid input or an invalid resulting record; exit code 2.
    Usage,
    /// Operational failure such as an unreadable tree; exit code 1.
    Failure,
}

/// A `bloomery spec` failure with its exit-code class.
#[derive(Debug, Clone)]
pub struct SpecError {
    kind: SpecErrorKind,
    message: String,
}

impl SpecError {
    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            kind: SpecErrorKind::Usage,
            message: message.into(),
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            kind: SpecErrorKind::Failure,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> SpecErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn exit_code(&self) -> u8 {
        match self.kind {
            SpecErrorKind::Usage => 2,
            SpecErrorKind::Failure => 1,
        }
    }

    pub(crate) fn from_diagnostics(diagnostics: Vec<Diagnostic>) -> Self {
        Self::failure(
            diagnostics
                .into_iter()
                .next()
                .map(|diagnostic| diagnostic.message)
                .unwrap_or_else(|| "unable to load the specification tree".to_owned()),
        )
    }

    pub(crate) fn from_diagnostic(diagnostic: Diagnostic) -> Self {
        Self::failure(diagnostic.message)
    }
}

/// One requirement record in the `spec list` projection.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecordSummary {
    pub area: String,
    pub feature: String,
    pub group: String,
    pub id: String,
    pub title: String,
    pub manual: bool,
    pub design_ref: String,
}

/// One requirement record in the `spec show` projection.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecordDetail {
    pub area: String,
    pub feature: String,
    pub group: String,
    pub id: String,
    pub title: String,
    pub manual: bool,
    pub design_ref: String,
    pub statement: String,
}

/// One statically discovered test reference tied to a requirement.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReferenceRecord {
    pub scanner: String,
    pub path: String,
    pub line: Option<usize>,
}

/// One `spec trace` record.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TraceRecord {
    pub id: String,
    pub area: String,
    pub feature: String,
    pub group: String,
    pub title: String,
    pub references: Vec<ReferenceRecord>,
}

/// One untied test site in the `spec candidates` projection.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CandidateRecord {
    pub scanner: String,
    pub name: Option<String>,
    pub path: String,
    pub line: Option<usize>,
}

/// Command-specific result.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum SpecOutcome {
    List {
        records: Vec<RecordSummary>,
    },
    Show {
        record: RecordDetail,
    },
    Add {
        id: String,
        path: String,
        created_group: bool,
    },
    Set {
        id: String,
        field: String,
        changed: bool,
    },
    Remove {
        id: String,
        pruned_group: bool,
    },
    Trace {
        requirements: Vec<TraceRecord>,
    },
    Candidates {
        candidates: Vec<CandidateRecord>,
    },
}

/// Successful result of one `bloomery spec` command.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpecReport {
    pub outcome: SpecOutcome,
}

/// Execute one `bloomery spec` command against the repository rooted at `root`.
pub fn run_at(operation: &SpecOperation, root: &Path) -> Result<SpecReport, SpecError> {
    let context = bloomery_workspace::load(root).map_err(SpecError::from_diagnostics)?;
    let outcome = match operation {
        SpecOperation::List {
            area,
            feature,
            group,
        } => SpecOutcome::List {
            records: editing::list(
                &context,
                area.as_deref(),
                feature.as_deref(),
                group.as_deref(),
            ),
        },
        SpecOperation::Show { id } => SpecOutcome::Show {
            record: editing::show(&context, id)?,
        },
        SpecOperation::Add {
            id,
            title,
            ears,
            design,
            manual,
        } => editing::add(root, &context, id, title, ears, design.as_deref(), *manual)?,
        SpecOperation::Set { id, field, value } => editing::set(root, &context, id, field, value)?,
        SpecOperation::Remove { id } => editing::remove(root, &context, id)?,
        SpecOperation::Trace { ids } => trace::trace(&context, root, ids)?,
        SpecOperation::Candidates => trace::candidates(&context, root)?,
    };
    Ok(SpecReport { outcome })
}

/// Find one declared requirement by exact ID.
pub(crate) fn find_requirement<'a>(
    context: &'a Context,
    id: &str,
) -> Result<
    (
        &'a bloomery_model::Area,
        &'a bloomery_model::Feature,
        &'a Requirement,
    ),
    SpecError,
> {
    context
        .requirements()
        .find(|(_, _, requirement)| requirement.entry.id == id)
        .ok_or_else(|| SpecError::usage(format!("unknown requirement ID '{id}'")))
}

/// Repository-relative display path using `/` separators.
pub(crate) fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
#[cfg(test)]
mod tests {
    use super::*;
    use bloomery_cli_types::SpecOperation;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    const AREA_README: &str =
        "---\nid: CLI\nname: CLI\ntagline: CLI\ndescription: Area\n---\n# CLI\n";
    const FEATURE_README: &str =
        "---\nid: SPEC\nname: Spec\ntagline: Spec\ndescription: Feature\n---\n# Spec\n";

    fn root(name: &str) -> PathBuf {
        let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "bloomery-spec-{name}-{}-{suffix}",
            std::process::id()
        ))
    }

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory");
        }
        fs::write(path, contents).expect("write fixture file");
    }

    fn record(id: &str, title: &str, manual: bool) -> String {
        format!(
            "[[requirements]]\nid = \"{id}\"\ntitle = \"{title}\"\nmanual = {manual}\ndesign = \"design/editing.md\"\n\n[requirements.ears]\ntype = \"ubiquitous\"\nsystem = \"s\"\naction = \"a\"\n"
        )
    }

    fn group_file(root: &Path, group: &str, records: &str) -> PathBuf {
        let path = root.join(format!(
            ".bloomery/specs/CLI/SPEC/requirements/{group}.toml"
        ));
        write(&path, &format!("group = \"{group}\"\n\n{records}"));
        path
    }

    fn workspace(name: &str) -> PathBuf {
        let root = root(name);
        write(&root.join("flake.nix"), "{ }\n");
        write(
            &root.join(".bloomery/config.toml"),
            "[specs]\ndir = \"specs\"\n",
        );
        write(&root.join(".bloomery/specs/CLI/README.md"), AREA_README);
        write(
            &root.join(".bloomery/specs/CLI/SPEC/README.md"),
            FEATURE_README,
        );
        write(
            &root.join(".bloomery/specs/CLI/SPEC/design/editing.md"),
            "# Editing\n\n## Additions\n\n## Reads\n",
        );
        group_file(
            &root,
            "TRACE",
            &record("CLI-SPEC-TRACE-001", "Trace", false),
        );
        root
    }

    fn run(operation: SpecOperation, root: &Path) -> Result<SpecReport, SpecError> {
        run_at(&operation, root)
    }

    fn current_dir_ears() -> &'static str {
        "{ type = \"ubiquitous\", system = \"s\", action = \"a\" }"
    }

    fn add(id: &str, title: &str) -> SpecOperation {
        SpecOperation::Add {
            id: id.to_owned(),
            title: title.to_owned(),
            ears: current_dir_ears().to_owned(),
            design: Some("design/editing.md".to_owned()),
            manual: false,
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-001"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-003"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-020"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-COMMANDS-001"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-COMMANDS-002"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-COMMANDS-003"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-COMMANDS-004"))]
    fn add_creates_a_group_and_rejects_duplicates() {
        let root = workspace("add");
        let report = run(add("CLI-SPEC-ADDED-001", "Added"), &root).expect("add");
        match report.outcome {
            SpecOutcome::Add {
                id, created_group, ..
            } => {
                assert_eq!(id, "CLI-SPEC-ADDED-001");
                assert!(created_group);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        let error = run(add("CLI-SPEC-ADDED-001", "Again"), &root).expect_err("duplicate");
        assert_eq!(error.kind(), SpecErrorKind::Usage);
        assert!(error.message().contains("already exists"));

        let shown = run(
            SpecOperation::Show {
                id: "CLI-SPEC-ADDED-001".to_owned(),
            },
            &root,
        )
        .expect("show");
        match shown.outcome {
            SpecOutcome::Show { record } => {
                assert_eq!(record.title, "Added");
                assert_eq!(record.statement, "The s shall a.");
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-002"))]
    fn add_requires_existing_architecture_documents() {
        let root = workspace("missing-docs");
        let error = run(add("ZZZ-AREA-GRP-001", "Missing"), &root).expect_err("area README");
        assert_eq!(error.kind(), SpecErrorKind::Usage);
        assert!(error.message().contains("README"));
        let error = run(add("CLI-NEW-GRP-001", "Missing"), &root).expect_err("feature README");
        assert_eq!(error.kind(), SpecErrorKind::Usage);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-004"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-021"))]
    fn invalid_and_unknown_ids_are_usage_errors() {
        let root = workspace("bad-id");
        let error = run(add("not-an-id", "Bad"), &root).expect_err("invalid ID");
        assert_eq!(error.kind(), SpecErrorKind::Usage);

        let error = run(
            SpecOperation::Show {
                id: "CLI-SPEC-NOPE-001".to_owned(),
            },
            &root,
        )
        .expect_err("unknown ID");
        assert_eq!(error.kind(), SpecErrorKind::Usage);
        assert!(error.message().contains("unknown"));
        let error = run(
            SpecOperation::Set {
                id: "CLI-SPEC-NOPE-001".to_owned(),
                field: "title".to_owned(),
                value: "x".to_owned(),
            },
            &root,
        )
        .expect_err("unknown ID");
        assert_eq!(error.kind(), SpecErrorKind::Usage);
        let error = run(
            SpecOperation::Remove {
                id: "CLI-SPEC-NOPE-001".to_owned(),
            },
            &root,
        )
        .expect_err("unknown ID");
        assert_eq!(error.kind(), SpecErrorKind::Usage);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-005"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-006"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-011"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-012"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-013"))]
    fn invalid_mutations_leave_the_tree_unchanged() {
        let root = workspace("invalid");
        let target = root.join(".bloomery/specs/CLI/SPEC/requirements/INVALID.toml");

        let bad_ears = SpecOperation::Add {
            id: "CLI-SPEC-INVALID-001".to_owned(),
            title: "Bad".to_owned(),
            ears: "{ type = \"bogus\", system = \"s\", action = \"a\" }".to_owned(),
            design: None,
            manual: false,
        };
        let error = run(bad_ears, &root).expect_err("invalid EARS");
        assert_eq!(error.kind(), SpecErrorKind::Usage);
        assert!(!target.exists(), "failed add must not create a group file");

        let missing_field = SpecOperation::Add {
            id: "CLI-SPEC-INVALID-001".to_owned(),
            title: "Bad".to_owned(),
            ears: "{ type = \"event\", trigger = \"t\", system = \"s\" }".to_owned(),
            design: None,
            manual: false,
        };
        assert!(run(missing_field, &root).is_err());

        let bad_design = SpecOperation::Add {
            id: "CLI-SPEC-INVALID-002".to_owned(),
            title: "Bad".to_owned(),
            ears: current_dir_ears().to_owned(),
            design: Some("design/missing.md".to_owned()),
            manual: false,
        };
        let error = run(bad_design, &root).expect_err("missing design");
        assert_eq!(error.kind(), SpecErrorKind::Usage);
        assert!(!target.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-007"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-008"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-023"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-COMMANDS-005"))]
    fn set_replaces_fields_and_whole_ears() {
        let root = workspace("set");
        run(
            SpecOperation::Set {
                id: "CLI-SPEC-TRACE-001".to_owned(),
                field: "title".to_owned(),
                value: "Renamed".to_owned(),
            },
            &root,
        )
        .expect("set title");
        run(
            SpecOperation::Set {
                id: "CLI-SPEC-TRACE-001".to_owned(),
                field: "ears".to_owned(),
                value: "{ type = \"event\", trigger = \"t\", system = \"s\", action = \"a\" }"
                    .to_owned(),
            },
            &root,
        )
        .expect("set ears");
        run(
            SpecOperation::Set {
                id: "CLI-SPEC-TRACE-001".to_owned(),
                field: "design".to_owned(),
                value: String::new(),
            },
            &root,
        )
        .expect("clear design");

        let path = root.join(".bloomery/specs/CLI/SPEC/requirements/TRACE.toml");
        let contents = fs::read_to_string(&path).expect("group file");
        assert!(contents.contains("Renamed"));
        assert!(contents.contains("type = \"event\""));
        assert!(!contents.contains("type = \"ubiquitous\""));
        assert!(!contents.contains("design ="));
        let shown = run(
            SpecOperation::Show {
                id: "CLI-SPEC-TRACE-001".to_owned(),
            },
            &root,
        )
        .expect("show");
        if let SpecOutcome::Show { record } = shown.outcome {
            assert_eq!(record.statement, "When t, the s shall a.");
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-009"))]
    fn set_rejects_unknown_fields() {
        let root = workspace("set-field");
        let error = run(
            SpecOperation::Set {
                id: "CLI-SPEC-TRACE-001".to_owned(),
                field: "bogus".to_owned(),
                value: "x".to_owned(),
            },
            &root,
        )
        .expect_err("unknown field");
        assert_eq!(error.kind(), SpecErrorKind::Usage);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-010"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-022"))]
    fn mutations_preserve_unrelated_content_and_leave_locks() {
        let root = workspace("preserve");
        let path = group_file(
            &root,
            "TRACE",
            "# keep this comment\n[[requirements]]\nid = \"CLI-SPEC-TRACE-001\"\ntitle = \"Trace\"\nmanual = false\ndesign = \"design/editing.md\"\n\n[requirements.ears]\ntype = \"ubiquitous\"\nsystem = \"s\"\naction = \"a\"\n",
        );
        let before = fs::read_to_string(&path).expect("group file");
        run(add("CLI-SPEC-ADDED-002", "Added"), &root).expect("add");
        let after = fs::read_to_string(&path).expect("group file");
        assert!(after.contains("# keep this comment"));
        assert!(after.contains("CLI-SPEC-TRACE-001"));
        assert!(!before.contains("CLI-SPEC-ADDED-002"));
        assert!(!root.join("Cargo.lock").exists());
        assert!(!root.join("bloomery.lock").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-014"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-015"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-016"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-COMMANDS-006"))]
    fn remove_deletes_records_and_prunes_groups() {
        let root = workspace("remove");
        // A second group so one may be pruned.
        let second = group_file(
            &root,
            "OTHER",
            &record("CLI-SPEC-OTHER-001", "Other", false),
        );
        let report = run(
            SpecOperation::Remove {
                id: "CLI-SPEC-OTHER-001".to_owned(),
            },
            &root,
        )
        .expect("remove");
        match report.outcome {
            SpecOutcome::Remove { pruned_group, .. } => assert!(pruned_group),
            other => panic!("unexpected outcome: {other:?}"),
        }
        assert!(!second.exists(), "emptied group is pruned");

        // The last group of the feature cannot be emptied.
        let error = run(
            SpecOperation::Remove {
                id: "CLI-SPEC-TRACE-001".to_owned(),
            },
            &root,
        )
        .expect_err("last group");
        assert_eq!(error.kind(), SpecErrorKind::Usage);
        assert!(
            root.join(".bloomery/specs/CLI/SPEC/requirements/TRACE.toml")
                .exists()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-017"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-018"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-EDITING-019"))]
    fn list_orders_and_filters_records() {
        let root = workspace("list");
        group_file(
            &root,
            "ALPHA",
            &format!(
                "{}{}",
                record("CLI-SPEC-ALPHA-010", "Ten", false),
                record("CLI-SPEC-ALPHA-002", "Two", false)
            ),
        );
        let report = run(
            SpecOperation::List {
                area: None,
                feature: None,
                group: None,
            },
            &root,
        )
        .expect("list");
        let ids = match report.outcome {
            SpecOutcome::List { records } => records.into_iter().map(|r| r.id).collect::<Vec<_>>(),
            other => panic!("unexpected outcome: {other:?}"),
        };
        assert_eq!(
            ids,
            [
                "CLI-SPEC-ALPHA-002",
                "CLI-SPEC-ALPHA-010",
                "CLI-SPEC-TRACE-001"
            ]
        );

        let filtered = run(
            SpecOperation::List {
                area: Some("CLI".to_owned()),
                feature: Some("SPEC".to_owned()),
                group: Some("ALPHA".to_owned()),
            },
            &root,
        )
        .expect("filtered list");
        match filtered.outcome {
            SpecOutcome::List { records } => assert_eq!(records.len(), 2),
            other => panic!("unexpected outcome: {other:?}"),
        }

        let empty = run(
            SpecOperation::List {
                area: None,
                feature: None,
                group: Some("NOPE".to_owned()),
            },
            &root,
        )
        .expect("unmatched filter");
        match empty.outcome {
            SpecOutcome::List { records } => assert!(records.is_empty()),
            other => panic!("unexpected outcome: {other:?}"),
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SPEC-TRACE-001"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-TRACE-002"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-TRACE-005"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-TRACE-012"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-TRACE-013"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-TRACE-014"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-TRACE-015"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-COMMANDS-007"))]
    fn trace_reports_tied_tests() {
        let root = workspace("trace");
        write(
            &root.join("packages/rust/a.rs"),
            "#[cfg_attr(any(), bloomery(\"CLI-SPEC-TRACE-001\"))]\n#[test]\nfn tied() { panic!(\"never executed\") }\n",
        );
        let before =
            fs::read_to_string(root.join(".bloomery/specs/CLI/SPEC/requirements/TRACE.toml"))
                .expect("group");
        let report = run(
            SpecOperation::Trace {
                ids: vec!["CLI-SPEC-TRACE-001".to_owned()],
            },
            &root,
        )
        .expect("trace");
        match report.outcome {
            SpecOutcome::Trace { requirements } => {
                assert_eq!(requirements.len(), 1);
                assert_eq!(requirements[0].references.len(), 1);
                assert_eq!(requirements[0].references[0].scanner, "rust");
                assert_eq!(requirements[0].references[0].path, "packages/rust/a.rs");
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        let after =
            fs::read_to_string(root.join(".bloomery/specs/CLI/SPEC/requirements/TRACE.toml"))
                .expect("group");
        assert_eq!(before, after);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SPEC-TRACE-003"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-TRACE-004"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-TRACE-011"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-TRACE-016"))]
    fn trace_succeeds_without_tests_and_rejects_unknown_ids() {
        let root = workspace("trace-empty");
        write(&root.join("lib/untagged.test.nix"), "{ }\n");
        let report = run(
            SpecOperation::Trace {
                ids: vec!["CLI-SPEC-TRACE-001".to_owned()],
            },
            &root,
        )
        .expect("trace");
        match report.outcome {
            SpecOutcome::Trace { requirements } => assert!(requirements[0].references.is_empty()),
            other => panic!("unexpected outcome: {other:?}"),
        }

        let candidates = run(SpecOperation::Candidates, &root).expect("candidates");
        match candidates.outcome {
            SpecOutcome::Candidates { candidates } => {
                assert_eq!(candidates.len(), 1);
                assert_eq!(candidates[0].scanner, "nix");
                assert_eq!(candidates[0].path, "lib/untagged.test.nix");
            }
            other => panic!("unexpected outcome: {other:?}"),
        }

        let error = run(
            SpecOperation::Trace {
                ids: vec!["CLI-SPEC-NOPE-001".to_owned()],
            },
            &root,
        )
        .expect_err("unknown trace ID");
        assert_eq!(error.kind(), SpecErrorKind::Usage);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SPEC-TRACE-006"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-TRACE-007"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-TRACE-008"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-TRACE-009"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-TRACE-010"))]
    #[cfg_attr(any(), bloomery("CLI-SPEC-COMMANDS-008"))]
    fn candidates_list_only_untied_tests() {
        let root = workspace("candidates");
        write(
            &root.join("packages/rust/a.rs"),
            "#[cfg_attr(any(), bloomery(\"CLI-SPEC-TRACE-001\"))]\n#[test]\nfn tied() {}\n\n#[test]\nfn untied() {}\n",
        );
        let before =
            fs::read_to_string(root.join(".bloomery/specs/CLI/SPEC/requirements/TRACE.toml"))
                .expect("group");
        let report = run(SpecOperation::Candidates, &root).expect("candidates");
        match report.outcome {
            SpecOutcome::Candidates { candidates } => {
                assert_eq!(candidates.len(), 1);
                assert_eq!(candidates[0].name.as_deref(), Some("untied"));
                assert_eq!(candidates[0].path, "packages/rust/a.rs");
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        let after =
            fs::read_to_string(root.join(".bloomery/specs/CLI/SPEC/requirements/TRACE.toml"))
                .expect("group");
        assert_eq!(before, after, "candidates never create requirements");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SPEC-COMMANDS-009"))]
    fn spec_uses_the_configured_specs_root() {
        let root = root("custom-root");
        write(&root.join("flake.nix"), "{ }\n");
        write(
            &root.join(".bloomery/config.toml"),
            "[specs]\ndir = \"custom\"\n",
        );
        write(&root.join(".bloomery/custom/CLI/README.md"), AREA_README);
        write(
            &root.join(".bloomery/custom/CLI/SPEC/README.md"),
            FEATURE_README,
        );
        write(
            &root.join(".bloomery/custom/CLI/SPEC/design/editing.md"),
            "# Editing\n",
        );
        write(
            &root.join(".bloomery/custom/CLI/SPEC/requirements/TRACE.toml"),
            &format!(
                "group = \"TRACE\"\n\n{}",
                record("CLI-SPEC-TRACE-001", "Trace", false)
            ),
        );
        let report = run(
            SpecOperation::List {
                area: None,
                feature: None,
                group: None,
            },
            &root,
        )
        .expect("list");
        match report.outcome {
            SpecOutcome::List { records } => assert_eq!(records.len(), 1),
            other => panic!("unexpected outcome: {other:?}"),
        }
        run(add("CLI-SPEC-CUSTOM-001", "Custom"), &root).expect("add");
        assert!(
            root.join(".bloomery/custom/CLI/SPEC/requirements/CUSTOM.toml")
                .is_file()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SPEC-COMMANDS-010"))]
    fn read_commands_do_not_scan_source() {
        let root = workspace("read-no-scan");
        write(&root.join("packages/rust/broken.rs"), "fn broken( {");
        run(
            SpecOperation::List {
                area: None,
                feature: None,
                group: None,
            },
            &root,
        )
        .expect("list must not scan");
        run(
            SpecOperation::Show {
                id: "CLI-SPEC-TRACE-001".to_owned(),
            },
            &root,
        )
        .expect("show must not scan");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-SPEC-COMMANDS-011"))]
    fn mutations_do_not_scan_source() {
        let root = workspace("mutate-no-scan");
        write(&root.join("packages/rust/broken.rs"), "fn broken( {");
        run(add("CLI-SPEC-ADDED-003", "Added"), &root).expect("add must not scan");
        run(
            SpecOperation::Set {
                id: "CLI-SPEC-ADDED-003".to_owned(),
                field: "title".to_owned(),
                value: "Renamed".to_owned(),
            },
            &root,
        )
        .expect("set must not scan");
        run(
            SpecOperation::Remove {
                id: "CLI-SPEC-ADDED-003".to_owned(),
            },
            &root,
        )
        .expect("remove must not scan");
        let _ = fs::remove_dir_all(root);
    }
}
