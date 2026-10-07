use bloomery_cli_types::{
    CheckArgs, CheckOperation, CliCommand, CliInvocation, ConfigArgs, ConfigOperation,
    DEFAULT_PAGE_LIMIT, DETAIL_PAGE_LIMIT, DetailsArgs, DiskArgs, DiskScope, FailureArgs, InitArgs,
    ListArgs, SpecArgs, SpecOperation,
};
use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum, error::ErrorKind};
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrorKind {
    DisplayHelp,
    DisplayVersion,
    Usage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    kind: ParseErrorKind,
    exit_code: u8,
    json_mode: bool,
    command: Option<&'static str>,
    message: String,
}

impl ParseError {
    pub fn kind(&self) -> ParseErrorKind {
        self.kind
    }

    pub fn exit_code(&self) -> u8 {
        self.exit_code
    }

    pub fn json_mode(&self) -> bool {
        self.json_mode
    }

    pub fn command(&self) -> Option<&'static str> {
        self.command
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

/// Parse process-style arguments into a command request without executing it.
pub fn parse_from<I, T>(arguments: I) -> Result<CliInvocation, ParseError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let arguments = arguments
        .into_iter()
        .map(Into::into)
        .collect::<Vec<OsString>>();
    let json_mode = requests_json_output(&arguments);
    let command = command_from_arguments(&arguments);

    let parsed = RawCli::try_parse_from(arguments).map_err(|error| ParseError {
        kind: match error.kind() {
            ErrorKind::DisplayHelp => ParseErrorKind::DisplayHelp,
            ErrorKind::DisplayVersion => ParseErrorKind::DisplayVersion,
            _ => ParseErrorKind::Usage,
        },
        exit_code: error.exit_code() as u8,
        json_mode,
        command,
        message: error.to_string(),
    })?;

    Ok(CliInvocation {
        json_mode: parsed.json,
        command: parsed.command.into(),
    })
}

fn requests_json_output(arguments: &[OsString]) -> bool {
    arguments
        .iter()
        .any(|argument| argument == "--json" || argument.to_string_lossy().starts_with("--json="))
}

fn command_from_arguments(arguments: &[OsString]) -> Option<&'static str> {
    arguments
        .iter()
        .find_map(|argument| match argument.to_str()? {
            "check" => Some("check"),
            "disk" => Some("disk"),
            "review" => Some("review"),
            "sync" => Some("sync"),
            "init" => Some("init"),
            "config" => Some("config"),
            "spec" => Some("spec"),
            _ => None,
        })
}

#[derive(Debug, Parser)]
#[command(
    name = "bloomery",
    about = "Workspace checks, requirements review, and lockfile synchronization"
)]
struct RawCli {
    /// Emit machine-readable JSON instead of human-readable output.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: RawCommand,
}

#[derive(Debug, Subcommand)]
enum RawCommand {
    /// Scaffold a new Bloomery flake and Rust workspace from a template.
    Init(RawInitArgs),
    /// Run static validation and selected Nix checks, or retrieve retained results.
    Check(RawCheckArgs),
    /// Report the Nix store size of the Bloomery tree or one derivation.
    Disk(RawDiskArgs),
    /// Print requirements that require human review.
    Review,
    /// Reconcile Cargo and Bloomery locks, optionally updating dependencies.
    Sync {
        /// Update all applicable ecosystems, or a comma-separated list of nix and rust.
        #[arg(
            long,
            value_name = "LIST",
            num_args = 0..=1,
            default_missing_value = "__bloomery_bare_update__",
            action = ArgAction::Set
        )]
        update: Option<String>,
    },
    /// Read, edit, document, or upgrade .bloomery/config.toml.
    Config(RawConfigArgs),
    /// List, edit, and trace repository specification records.
    Spec(RawSpecArgs),
}

impl From<RawCommand> for CliCommand {
    fn from(command: RawCommand) -> Self {
        match command {
            RawCommand::Init(args) => Self::Init(args.into()),
            RawCommand::Check(args) => Self::Check(args.into()),
            RawCommand::Disk(args) => Self::Disk(args.into()),
            RawCommand::Review => Self::Review,
            RawCommand::Sync { update } => Self::Sync { update },
            RawCommand::Config(args) => Self::Config(args.into()),
            RawCommand::Spec(args) => Self::Spec(args.into()),
        }
    }
}

#[derive(Debug, Args)]
struct RawConfigArgs {
    #[command(subcommand)]
    operation: RawConfigOperation,
}
#[derive(Debug, Subcommand)]
enum RawConfigOperation {
    /// Read one configuration key.
    Get {
        /// Dotted key path such as checks.enable.
        key: String,
    },
    /// Set a configuration key value.
    Set {
        /// Dotted key path such as checks.enable.
        key: String,
        /// Value parsed according to the key's schema type.
        value: String,
    },
    /// Remove a configuration key.
    Unset {
        /// Dotted key path such as checks.enable.
        key: String,
    },
    /// List configuration keys.
    List {
        /// Restrict to keys whose dotted path starts with this prefix.
        #[arg(long, value_name = "KEY")]
        prefix: Option<String>,
    },
    /// Add absent recommended keys or compare recommendations.
    Upgrade {
        /// Report additions without writing.
        #[arg(long, conflicts_with = "diff")]
        dry_run: bool,
        /// Compare recommendations with the current configuration without writing.
        #[arg(long)]
        diff: bool,
    },
    /// Annotate configured keys with schema documentation.
    Document,
}

impl From<RawConfigArgs> for ConfigArgs {
    fn from(args: RawConfigArgs) -> Self {
        Self {
            operation: args.operation.into(),
        }
    }
}

impl From<RawConfigOperation> for ConfigOperation {
    fn from(operation: RawConfigOperation) -> Self {
        match operation {
            RawConfigOperation::Get { key } => Self::Get { key },
            RawConfigOperation::Set { key, value } => Self::Set { key, value },
            RawConfigOperation::Unset { key } => Self::Unset { key },
            RawConfigOperation::List { prefix } => Self::List { prefix },
            RawConfigOperation::Upgrade { dry_run, diff } => Self::Upgrade { dry_run, diff },
            RawConfigOperation::Document => Self::Document,
        }
    }
}

#[derive(Debug, Args)]
struct RawSpecArgs {
    #[command(subcommand)]
    operation: RawSpecOperation,
}

#[derive(Debug, Subcommand)]
enum RawSpecOperation {
    /// List requirement records.
    List {
        /// Restrict to one area identifier.
        #[arg(long, value_name = "AREA")]
        area: Option<String>,
        /// Restrict to one feature identifier.
        #[arg(long, value_name = "FEATURE")]
        feature: Option<String>,
        /// Restrict to one requirement group.
        #[arg(long, value_name = "GROUP")]
        group: Option<String>,
    },
    /// Show one requirement record.
    Show {
        /// Full requirement ID.
        id: String,
    },
    /// Create a requirement record.
    Add {
        /// Full requirement ID; its segments derive the target file.
        id: String,
        /// Human-readable record title.
        #[arg(long, value_name = "TITLE")]
        title: String,
        /// EARS clause as a TOML inline table.
        #[arg(long, value_name = "TOML")]
        ears: String,
        /// Feature-relative design path with an optional anchor.
        #[arg(long, value_name = "PATH")]
        design: Option<String>,
        /// Mark the record as human-reviewed instead of automated.
        #[arg(long)]
        manual: bool,
    },
    /// Replace one field of a requirement record.
    Set {
        /// Full requirement ID.
        id: String,
        /// Field to replace: title, manual, design, or ears.
        field: String,
        /// New field value.
        value: String,
    },
    /// Delete a requirement record.
    Remove {
        /// Full requirement ID.
        id: String,
    },
    /// Report the tests statically tied to one or more requirements.
    Trace {
        /// Full requirement IDs.
        #[arg(required = true, num_args = 1.., value_name = "ID")]
        ids: Vec<String>,
    },
    /// Report discovered tests that are not tied to any requirement.
    Candidates,
}

impl From<RawSpecArgs> for SpecArgs {
    fn from(args: RawSpecArgs) -> Self {
        Self {
            operation: args.operation.into(),
        }
    }
}

impl From<RawSpecOperation> for SpecOperation {
    fn from(operation: RawSpecOperation) -> Self {
        match operation {
            RawSpecOperation::List {
                area,
                feature,
                group,
            } => Self::List {
                area,
                feature,
                group,
            },
            RawSpecOperation::Show { id } => Self::Show { id },
            RawSpecOperation::Add {
                id,
                title,
                ears,
                design,
                manual,
            } => Self::Add {
                id,
                title,
                ears,
                design,
                manual,
            },
            RawSpecOperation::Set { id, field, value } => Self::Set { id, field, value },
            RawSpecOperation::Remove { id } => Self::Remove { id },
            RawSpecOperation::Trace { ids } => Self::Trace { ids },
            RawSpecOperation::Candidates => Self::Candidates,
        }
    }
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
struct RawDiskArgs {
    /// Derivation to measure: a flake output attribute path or a Nix store path.
    derivation: Option<String>,
    /// Select the runtime packages, the build checks, or both.
    #[arg(
        long = "scope",
        value_name = "SCOPE",
        value_enum,
        default_value = "all",
        conflicts_with = "derivation"
    )]
    scope: RawDiskScope,
    /// Select a Nix system for a full-tree measurement; may be repeated.
    #[arg(
        long = "system",
        value_name = "SYSTEM",
        action = ArgAction::Append,
        conflicts_with = "derivation"
    )]
    systems: Vec<String>,
    #[command(subcommand)]
    operation: Option<RawDiskOperation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum RawDiskScope {
    All,
    Runtime,
    Build,
}

impl From<RawDiskScope> for DiskScope {
    fn from(scope: RawDiskScope) -> Self {
        match scope {
            RawDiskScope::All => Self::All,
            RawDiskScope::Runtime => Self::Runtime,
            RawDiskScope::Build => Self::Build,
        }
    }
}

#[derive(Debug, Subcommand)]
enum RawDiskOperation {
    /// Render the category tree instead of the size summary.
    Tree(RawDiskTreeArgs),
}

#[derive(Debug, Args)]
struct RawDiskTreeArgs {
    /// Derivation to measure: a flake output attribute path or a Nix store path.
    derivation: Option<String>,
    /// Select the runtime packages, the build checks, or both.
    #[arg(
        long = "scope",
        value_name = "SCOPE",
        value_enum,
        default_value = "all",
        conflicts_with = "derivation"
    )]
    scope: RawDiskScope,
    /// Select a Nix system for a full-tree measurement; may be repeated.
    #[arg(
        long = "system",
        value_name = "SYSTEM",
        action = ArgAction::Append,
        conflicts_with = "derivation"
    )]
    systems: Vec<String>,
}

impl From<RawDiskArgs> for DiskArgs {
    fn from(args: RawDiskArgs) -> Self {
        match args.operation {
            Some(RawDiskOperation::Tree(tree)) => Self {
                tree: true,
                scope: tree.scope.into(),
                derivation: tree.derivation,
                systems: deduplicate_systems(tree.systems),
            },
            None => Self {
                tree: false,
                scope: args.scope.into(),
                derivation: args.derivation,
                systems: deduplicate_systems(args.systems),
            },
        }
    }
}

fn deduplicate_systems(systems: Vec<String>) -> Vec<String> {
    let mut deduplicated = Vec::new();
    for system in systems {
        if !deduplicated.contains(&system) {
            deduplicated.push(system);
        }
    }
    deduplicated
}

#[derive(Debug, Args)]
struct RawInitArgs {
    /// Target directory; defaults to the current working directory.
    directory: Option<PathBuf>,
    /// Template to scaffold; defaults to the basic workspace template.
    #[arg(long, value_name = "NAME")]
    template: Option<String>,
    /// Overwrite files in a non-empty target directory.
    #[arg(long)]
    force: bool,
}

impl From<RawInitArgs> for InitArgs {
    fn from(args: RawInitArgs) -> Self {
        Self {
            directory: args.directory,
            template: args.template,
            force: args.force,
        }
    }
}

#[derive(Debug, Args)]
struct RawCheckArgs {
    #[command(subcommand)]
    operation: Option<RawCheckOperation>,

    /// Select checks by exact ID or case-sensitive * and ? glob.
    #[arg(long = "check", value_name = "ID_OR_GLOB", action = ArgAction::Append)]
    selectors: Vec<String>,

    /// Select a Nix system; may be repeated.
    #[arg(long = "system", value_name = "SYSTEM", action = ArgAction::Append)]
    systems: Vec<String>,

    /// Maximum number of Bloomery-owned check tasks running at once.
    #[arg(long, value_name = "N")]
    jobs: Option<usize>,

    /// Stop admitting checks after the first observed check failure.
    #[arg(long)]
    fail_fast: bool,
}

impl From<RawCheckArgs> for CheckArgs {
    fn from(args: RawCheckArgs) -> Self {
        Self {
            operation: args.operation.map(Into::into),
            selectors: args.selectors,
            systems: args.systems,
            jobs: args.jobs,
            fail_fast: args.fail_fast,
        }
    }
}

#[derive(Debug, Subcommand)]
enum RawCheckOperation {
    /// List selectable check IDs without running them.
    List(RawListArgs),
    /// Page through failures from a retained run.
    Failures(RawFailureArgs),
    /// Retrieve the retained details for one failure ID.
    Details(RawDetailsArgs),
}

impl From<RawCheckOperation> for CheckOperation {
    fn from(operation: RawCheckOperation) -> Self {
        match operation {
            RawCheckOperation::List(args) => Self::List(args.into()),
            RawCheckOperation::Failures(args) => Self::Failures(args.into()),
            RawCheckOperation::Details(args) => Self::Details(args.into()),
        }
    }
}

#[derive(Debug, Args)]
struct RawListArgs {
    /// Filter IDs by exact selector or case-sensitive * and ? glob.
    #[arg(long = "check", value_name = "ID_OR_GLOB", action = ArgAction::Append)]
    selectors: Vec<String>,
    /// Select a Nix system; may be repeated.
    #[arg(long = "system", value_name = "SYSTEM", action = ArgAction::Append)]
    systems: Vec<String>,
    /// Zero-based ID offset.
    #[arg(long, default_value_t = 0)]
    offset: usize,
    /// Maximum number of IDs to return.
    #[arg(long, default_value_t = DEFAULT_PAGE_LIMIT)]
    limit: usize,
}

impl From<RawListArgs> for ListArgs {
    fn from(args: RawListArgs) -> Self {
        Self {
            selectors: args.selectors,
            systems: args.systems,
            offset: args.offset,
            limit: args.limit,
        }
    }
}

#[derive(Debug, Args)]
struct RawFailureArgs {
    /// Retained run ID; omitted selects the latest completed run.
    #[arg(long, value_name = "RUN")]
    run: Option<String>,
    /// Zero-based failure offset.
    #[arg(long, default_value_t = 0)]
    offset: usize,
    /// Maximum number of failure records to return.
    #[arg(long, default_value_t = DEFAULT_PAGE_LIMIT)]
    limit: usize,
}

impl From<RawFailureArgs> for FailureArgs {
    fn from(args: RawFailureArgs) -> Self {
        Self {
            run: args.run,
            offset: args.offset,
            limit: args.limit,
        }
    }
}

#[derive(Debug, Args)]
struct RawDetailsArgs {
    /// Failure ID such as f1.
    failure: String,
    /// Retained run ID; omitted selects the latest completed run.
    #[arg(long, value_name = "RUN")]
    run: Option<String>,
    /// Zero-based detail-record offset; omitted selects a failure-focused excerpt.
    #[arg(long)]
    offset: Option<usize>,
    /// Maximum number of display records to return.
    #[arg(long, default_value_t = DETAIL_PAGE_LIMIT)]
    limit: usize,
}

impl From<RawDetailsArgs> for DetailsArgs {
    fn from(args: RawDetailsArgs) -> Self {
        Self {
            failure: args.failure,
            run: args.run,
            offset: args.offset,
            limit: args.limit,
        }
    }
}
