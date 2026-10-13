use bloomery_cli_types::{
    CheckOperation, CliCommand, ConfigArgs, ConfigOperation, DetailsArgs, SpecArgs, SpecOperation,
};
use std::collections::BTreeSet;

/// Every command path the CLI currently exposes.
///
/// The exhaustive matches below make a new CLI variant a compile error here, so
/// the documentation catalog cannot silently drift from the parser model.
pub fn command_paths() -> BTreeSet<String> {
    let mut paths = BTreeSet::new();

    let commands = [
        CliCommand::Init(Default::default()),
        CliCommand::Check(Default::default()),
        CliCommand::Build(Default::default()),
        CliCommand::Disk(Default::default()),
        CliCommand::Review,
        CliCommand::Sync { update: None },
        CliCommand::Config(ConfigArgs {
            operation: ConfigOperation::List { prefix: None },
        }),
        CliCommand::Spec(SpecArgs {
            operation: SpecOperation::List {
                area: None,
                feature: None,
                group: None,
            },
        }),
    ];
    for command in &commands {
        paths.insert(command_name(command).to_string());
    }
    // `disk tree` is a subcommand represented by a boolean flag in the model.
    paths.insert("disk tree".to_string());

    let check_operations = [
        CheckOperation::List(Default::default()),
        CheckOperation::Failures(Default::default()),
        CheckOperation::Details(DetailsArgs {
            failure: String::new(),
            run: None,
            offset: None,
            limit: 0,
        }),
    ];
    for operation in &check_operations {
        paths.insert(check_operation_path(operation).to_string());
    }

    let config_operations = [
        ConfigOperation::Get { key: String::new() },
        ConfigOperation::Set {
            key: String::new(),
            value: String::new(),
        },
        ConfigOperation::Unset { key: String::new() },
        ConfigOperation::List { prefix: None },
        ConfigOperation::Upgrade {
            dry_run: false,
            diff: false,
        },
        ConfigOperation::Document,
    ];
    for operation in &config_operations {
        paths.insert(config_operation_path(operation).to_string());
    }

    let spec_operations = [
        SpecOperation::List {
            area: None,
            feature: None,
            group: None,
        },
        SpecOperation::Show { id: String::new() },
        SpecOperation::Add {
            id: String::new(),
            title: String::new(),
            ears: String::new(),
            design: None,
            manual: false,
        },
        SpecOperation::Set {
            id: String::new(),
            field: String::new(),
            value: String::new(),
        },
        SpecOperation::Remove { id: String::new() },
        SpecOperation::Trace { ids: Vec::new() },
        SpecOperation::Candidates,
    ];
    for operation in &spec_operations {
        paths.insert(spec_operation_path(operation).to_string());
    }

    paths
}

fn command_name(command: &CliCommand) -> &'static str {
    match command {
        CliCommand::Init(_) => "init",
        CliCommand::Check(_) => "check",
        CliCommand::Build(_) => "build",
        CliCommand::Disk(_) => "disk",
        CliCommand::Review => "review",
        CliCommand::Sync { .. } => "sync",
        CliCommand::Config(_) => "config",
        CliCommand::Spec(_) => "spec",
    }
}

fn check_operation_path(operation: &CheckOperation) -> &'static str {
    match operation {
        CheckOperation::List(_) => "check list",
        CheckOperation::Failures(_) => "check failures",
        CheckOperation::Details(_) => "check details",
    }
}

fn config_operation_path(operation: &ConfigOperation) -> &'static str {
    match operation {
        ConfigOperation::Get { .. } => "config get",
        ConfigOperation::Set { .. } => "config set",
        ConfigOperation::Unset { .. } => "config unset",
        ConfigOperation::List { .. } => "config list",
        ConfigOperation::Upgrade { .. } => "config upgrade",
        ConfigOperation::Document => "config document",
    }
}

fn spec_operation_path(operation: &SpecOperation) -> &'static str {
    match operation {
        SpecOperation::List { .. } => "spec list",
        SpecOperation::Show { .. } => "spec show",
        SpecOperation::Add { .. } => "spec add",
        SpecOperation::Set { .. } => "spec set",
        SpecOperation::Remove { .. } => "spec remove",
        SpecOperation::Trace { .. } => "spec trace",
        SpecOperation::Candidates => "spec candidates",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_representative_paths() {
        let paths = command_paths();
        assert!(paths.contains("check"));
        assert!(paths.contains("check list"));
        assert!(paths.contains("config document"));
        assert!(paths.contains("spec candidates"));
        assert!(paths.contains("disk tree"));
        assert!(!paths.contains("bogus"));
    }
}
