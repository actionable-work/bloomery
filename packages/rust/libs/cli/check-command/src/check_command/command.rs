use super::catalog::{NixBackend, NixCli, discover_catalog};
use super::execution::run_checks;
use super::interrupt::InterruptFlag;
use super::model::{Notice, selector_may_match_prefix, valid_system_name};
use super::output;
use super::retrieval::{run_details, run_failures, run_list};
pub use bloomery_cli_types::{CheckArgs, CheckOperation, DetailsArgs, FailureArgs, ListArgs};
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CheckError {
    pub(super) code: &'static str,
    pub(super) message: String,
}

pub(super) struct CheckContext<'a> {
    pub(super) root: &'a Path,
    pub(super) json_mode: bool,
    pub(super) backend: &'a dyn NixBackend,
    pub(super) interrupt: InterruptFlag,
    pub(super) cache_base: Option<&'a Path>,
}

impl CheckError {
    pub(super) fn usage(message: impl Into<String>) -> Self {
        Self {
            code: "UsageError",
            message: message.into(),
        }
    }

    pub(super) fn operational(message: impl Into<String>) -> Self {
        Self {
            code: "OperationalError",
            message: message.into(),
        }
    }

    pub(super) fn retrieval(message: impl Into<String>) -> Self {
        Self {
            code: "RetrievalError",
            message: message.into(),
        }
    }
}

pub fn run(args: CheckArgs, json_mode: bool) -> ExitCode {
    let root = match std::env::current_dir() {
        Ok(root) => root,
        Err(error) => {
            let mut stdout = io::stdout().lock();
            let mut stderr = io::stderr().lock();
            return report_error(
                json_mode,
                &CheckError::operational(format!("unable to determine workspace root: {error}")),
                None,
                &mut stdout,
                &mut stderr,
            );
        }
    };
    let interrupt = InterruptFlag::install();
    let backend = NixCli::default();
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    run_at_with(
        args,
        CheckContext {
            root: &root,
            json_mode,
            backend: &backend,
            interrupt,
            cache_base: None,
        },
        &mut stdout,
        &mut stderr,
    )
}

pub fn run_at(
    args: CheckArgs,
    json_mode: bool,
    root: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let backend = NixCli::default();
    run_at_with(
        args,
        CheckContext {
            root,
            json_mode,
            backend: &backend,
            interrupt: InterruptFlag::install(),
            cache_base: None,
        },
        stdout,
        stderr,
    )
}

pub fn run_at_with_cache(
    args: CheckArgs,
    json_mode: bool,
    root: &Path,
    cache_base: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let backend = NixCli::default();
    run_at_with(
        args,
        CheckContext {
            root,
            json_mode,
            backend: &backend,
            interrupt: InterruptFlag::install(),
            cache_base: Some(cache_base),
        },
        stdout,
        stderr,
    )
}

pub(super) fn run_at_with(
    args: CheckArgs,
    context: CheckContext<'_>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    if let Err(error) = validate_args(&args) {
        return report_error(context.json_mode, &error, None, stdout, stderr);
    }
    let CheckContext {
        root: requested_root,
        json_mode,
        backend,
        interrupt,
        cache_base,
    } = context;
    let root = match fs::canonicalize(requested_root) {
        Ok(root) => root,
        Err(error) => {
            return report_error(
                json_mode,
                &CheckError::operational(format!("unable to resolve workspace root: {error}")),
                None,
                stdout,
                stderr,
            );
        }
    };

    let context = CheckContext {
        root: &root,
        json_mode,
        backend,
        interrupt,
        cache_base,
    };
    let CheckArgs {
        operation,
        selectors,
        systems,
        jobs,
        fail_fast,
    } = args;
    match operation {
        Some(CheckOperation::List(list)) => run_list(
            context.root,
            list,
            json_mode,
            stdout,
            stderr,
            context.backend,
        ),
        Some(CheckOperation::Failures(failures)) => run_failures(
            context.root,
            failures,
            json_mode,
            stdout,
            stderr,
            context.cache_base,
        ),
        Some(CheckOperation::Details(details)) => run_details(
            context.root,
            details,
            json_mode,
            stdout,
            stderr,
            context.cache_base,
            context.backend,
        ),
        None => run_checks(
            CheckArgs {
                operation: None,
                selectors,
                systems,
                jobs,
                fail_fast,
            },
            &context,
            stdout,
            stderr,
        ),
    }
}

pub(super) fn validate_args(args: &CheckArgs) -> Result<(), CheckError> {
    if args.operation.is_some()
        && (!args.selectors.is_empty()
            || !args.systems.is_empty()
            || args.jobs.is_some()
            || args.fail_fast)
    {
        return Err(CheckError::usage(
            "execution options --check, --system, --jobs, and --fail-fast are not accepted before retrieval subcommands",
        ));
    }
    if args.jobs == Some(0) {
        return Err(CheckError::usage("--jobs must be a positive integer"));
    }
    for selector in &args.selectors {
        if selector.is_empty() {
            return Err(CheckError::usage("--check selectors must not be empty"));
        }
    }
    for system in &args.systems {
        if !valid_system_name(system) {
            return Err(CheckError::usage(format!(
                "invalid Nix system name '{system}'"
            )));
        }
    }
    match &args.operation {
        Some(CheckOperation::List(list)) => {
            validate_page_values(list.offset, list.limit)?;
            if list.selectors.iter().any(String::is_empty) {
                return Err(CheckError::usage("--check selectors must not be empty"));
            }
            for system in &list.systems {
                if !valid_system_name(system) {
                    return Err(CheckError::usage(format!(
                        "invalid Nix system name '{system}'"
                    )));
                }
            }
        }
        Some(CheckOperation::Failures(failures)) => {
            validate_page_values(failures.offset, failures.limit)?;
        }
        Some(CheckOperation::Details(details)) => {
            validate_page_values(details.offset.unwrap_or(0), details.limit)?;
            if details.failure.is_empty() {
                return Err(CheckError::usage("failure ID must not be empty"));
            }
        }
        None => {}
    }
    Ok(())
}

fn validate_page_values(_offset: usize, limit: usize) -> Result<(), CheckError> {
    if limit == 0 {
        return Err(CheckError::usage("--limit must be a positive integer"));
    }
    Ok(())
}

pub(super) struct Catalog {
    pub(super) ids: Vec<String>,
    pub(super) systems: Vec<String>,
    pub(super) notices: Vec<Notice>,
}

pub(super) fn discover_for_selection(
    root: &Path,
    selectors: &[String],
    requested_systems: &[String],
    backend: &dyn NixBackend,
) -> Result<Catalog, CheckError> {
    let has_flake = root.join("flake.nix").is_file();
    if !has_flake {
        if !requested_systems.is_empty() {
            return Err(CheckError::usage(
                "--system requires a workspace containing flake.nix",
            ));
        }
        if selectors
            .iter()
            .any(|selector| selector.starts_with("nix:"))
        {
            return Err(CheckError::usage(
                "Nix checks cannot be selected because the workspace has no flake.nix",
            ));
        }
        return Ok(Catalog {
            ids: vec![
                "static:structure".to_owned(),
                "static:traceability".to_owned(),
            ],
            systems: Vec::new(),
            notices: Vec::new(),
        });
    }

    if requested_systems.is_empty()
        && !selectors.is_empty()
        && selectors
            .iter()
            .all(|selector| !selector_may_match_prefix(selector, "nix:"))
    {
        return Ok(Catalog {
            ids: vec![
                "static:structure".to_owned(),
                "static:traceability".to_owned(),
            ],
            systems: Vec::new(),
            notices: Vec::new(),
        });
    }

    let systems = effective_systems(root, requested_systems, backend)?;
    let (ids, notices) =
        discover_catalog(root, &systems, backend).map_err(CheckError::operational)?;
    Ok(Catalog {
        ids,
        systems,
        notices,
    })
}

pub(super) fn effective_systems(
    root: &Path,
    requested_systems: &[String],
    backend: &dyn NixBackend,
) -> Result<Vec<String>, CheckError> {
    if requested_systems.is_empty() {
        return backend
            .host_system(root)
            .map(|system| vec![system])
            .map_err(CheckError::operational);
    }
    let systems = requested_systems
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if let Some(invalid) = systems.iter().find(|system| !valid_system_name(system)) {
        return Err(CheckError::usage(format!(
            "invalid Nix system name '{invalid}'"
        )));
    }
    Ok(systems)
}

pub(super) fn write_page(
    json_mode: bool,
    bytes: Vec<u8>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let bytes = if json_mode {
        bytes
    } else {
        crate::output::colorize_check(
            &String::from_utf8_lossy(&bytes),
            crate::output::color_enabled(crate::output::Stream::Stdout, false),
        )
        .into_bytes()
    };
    match stdout.write_all(&bytes).and_then(|()| stdout.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(stderr, "bloomery: unable to write check output: {error}");
            ExitCode::from(2)
        }
    }
}

pub(super) fn report_error(
    json_mode: bool,
    error: &CheckError,
    run: Option<&str>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    if json_mode {
        let mut document = output::error_document(error.code, &error.message);
        if let Some(run) = run {
            document["run"] = serde_json::Value::String(run.to_owned());
        }
        match output::compact_json(&document) {
            Ok(bytes) => {
                let _ = stdout.write_all(&bytes).and_then(|()| stdout.flush());
            }
            Err(message) => {
                let _ = writeln!(stderr, "bloomery: {message}");
            }
        }
    } else {
        let message = format!("error: {}", error.message);
        let rendered = crate::output::colorize_error(
            &message,
            crate::output::color_enabled(crate::output::Stream::Stderr, false),
        );
        let _ = writeln!(stderr, "{rendered}");
    }
    ExitCode::from(2)
}
