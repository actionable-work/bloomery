use bloomery_check_catalog::check_command::catalog::NixBackend;
use bloomery_check_output::check_command::{
    details::{diagnostic_detail_text, display_records, normalize_display_bytes},
    output::{self, DetailPage},
    render::{report_error, write_page},
};
use bloomery_check_selection::check_command::selection::discover_for_selection;
use bloomery_check_storage::check_command::store;
use bloomery_check_support::check_command::{
    context::{CheckError, HealthContext},
    model::{FailureRecord, RunRecord, select_ids},
};
use bloomery_cli_types::{DetailsArgs, FailureArgs, ListArgs};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub fn run_list(
    root: &Path,
    args: ListArgs,
    json_mode: bool,
    color: bloomery_cli_output::ColorMode,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    backend: &dyn NixBackend,
) -> ExitCode {
    let catalog = match discover_for_selection(root, &args.selectors, &args.systems, backend) {
        Ok(catalog) => catalog,
        Err(error) => return report_error(json_mode, &error, None, stdout, stderr),
    };
    let ids = match select_ids(&catalog.ids, &args.selectors) {
        Ok(ids) => ids,
        Err(message) => {
            return report_error(json_mode, &CheckError::usage(message), None, stdout, stderr);
        }
    };
    let page = match output::list_page(
        &ids,
        &catalog.systems,
        &catalog.notices,
        args.offset,
        args.limit,
    ) {
        Ok(page) => page,
        Err(message) => {
            return report_error(
                json_mode,
                &CheckError::operational(message),
                None,
                stdout,
                stderr,
            );
        }
    };
    let bytes = if json_mode {
        page.bytes.clone()
    } else {
        output::list_text(&ids, &catalog.systems, args.offset, &page).into_bytes()
    };
    write_page(json_mode, color, bytes, stdout, stderr)
}

pub fn run_failures(
    root: &Path,
    args: FailureArgs,
    json_mode: bool,
    color: bloomery_cli_output::ColorMode,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    cache_base: Option<&Path>,
) -> ExitCode {
    let store = match store::RunStore::open(root, cache_base) {
        Ok(store) => store,
        Err(message) => {
            return report_error(
                json_mode,
                &CheckError::operational(message),
                None,
                stdout,
                stderr,
            );
        }
    };
    let (record, _) = match load_run(&store, args.run.as_deref()) {
        Ok(record) => record,
        Err(error) => return report_error(json_mode, &error, None, stdout, stderr),
    };
    let page = output::failure_page(&record, args.offset, args.limit);
    let bytes = if json_mode {
        page.bytes
    } else {
        output::failures_text(&record, args.offset, &page).into_bytes()
    };
    write_page(json_mode, color, bytes, stdout, stderr)
}

pub fn run_details(
    args: DetailsArgs,
    context: &HealthContext<'_, dyn NixBackend + '_>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> ExitCode {
    let root = context.root;
    let store = match store::RunStore::open(root, context.cache_base) {
        Ok(store) => store,
        Err(message) => {
            return report_error(
                context.json_mode,
                &CheckError::operational(message),
                None,
                stdout,
                stderr,
            );
        }
    };
    let (record, directory) = match load_run(&store, args.run.as_deref()) {
        Ok(record) => record,
        Err(error) => return report_error(context.json_mode, &error, None, stdout, stderr),
    };
    let Some(failure) = record
        .failures
        .iter()
        .find(|failure| failure.id == args.failure)
    else {
        return report_error(
            context.json_mode,
            &CheckError::retrieval(format!(
                "failure '{}' is not available in run '{}'",
                args.failure, record.id
            )),
            Some(&record.id),
            stdout,
            stderr,
        );
    };
    let records = match detail_records(failure, &directory, root, context.backend) {
        Ok(records) => records,
        Err(message) => {
            return report_error(
                context.json_mode,
                &CheckError::retrieval(message),
                Some(&record.id),
                stdout,
                stderr,
            );
        }
    };
    let outcome = record
        .outcomes
        .iter()
        .find(|outcome| outcome.id == failure.check);
    let requested_offset = args.offset.unwrap_or_else(|| {
        if failure.focus_tail {
            output::failure_focused_offset(&record, failure, outcome, &records, args.limit)
        } else {
            0
        }
    });
    let detail_page = DetailPage {
        run: &record,
        failure,
        outcome,
        records: &records,
        requested_offset,
        limit: args.limit,
    };
    let page = output::details_page(DetailPage {
        run: detail_page.run,
        failure: detail_page.failure,
        outcome: detail_page.outcome,
        records: detail_page.records,
        requested_offset: detail_page.requested_offset,
        limit: detail_page.limit,
    });
    let bytes = if context.json_mode {
        page.bytes
    } else {
        output::details_text(&detail_page, &page).into_bytes()
    };
    write_page(context.json_mode, context.color, bytes, stdout, stderr)
}

fn load_run(
    store: &store::RunStore,
    requested_id: Option<&str>,
) -> Result<(RunRecord, PathBuf), CheckError> {
    let result = match requested_id {
        Some(id) => store.load(id),
        None => store.latest(),
    };
    result.map_err(CheckError::retrieval)
}

fn detail_records(
    failure: &FailureRecord,
    directory: &Path,
    root: &Path,
    backend: &dyn NixBackend,
) -> Result<Vec<String>, String> {
    let mut records = if let Some(log) = &failure.log {
        let path = store::resolve_log_path(directory, log)?;
        let bytes = fs::read(&path).map_err(|error| {
            format!(
                "retained detail log is unavailable: {} ({error})",
                path.display()
            )
        })?;
        normalize_display_bytes(&bytes)
    } else {
        display_records(&diagnostic_detail_text(
            &failure.code,
            &failure.message,
            failure.location.as_ref(),
            &failure.notes,
        ))
    };
    if let Some(nix_log) = &failure.nix_log {
        records.extend(display_records(&format!("Nix derivation log: {nix_log}")));
        let derivation_records = store::retain_derivation_records(directory, nix_log, || {
            match backend.read_derivation_log(root, nix_log) {
                Ok(log) => normalize_display_bytes(&log),
                Err(error) => display_records(
                    &bloomery_check_output::check_command::details::normalize_display_text(
                        &format!("Nix derivation log unavailable: {error}"),
                    ),
                ),
            }
        })?;
        records.extend(derivation_records);
    }
    if records.is_empty() {
        Ok(vec!["(no retained output)".to_owned()])
    } else {
        Ok(records)
    }
}
