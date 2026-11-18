use std::path::{Path, PathBuf};

/// Message for the root `flake.nix` preflight required by every command.
pub const MISSING_FLAKE_MESSAGE: &str = "A Bloomery flake.nix is required; set up a Bloomery flake.nix in the current directory before running this command.";

/// Stable error code shared by the CLI preflight and each command's own
/// missing-flake boundary check.
pub const MISSING_FLAKE_CODE: &str = "MissingFlake";

/// Whether `root` contains the Bloomery `flake.nix` every command requires.
pub fn flake_present(root: &Path) -> bool {
    root.join("flake.nix").is_file()
}

pub const DEFAULT_PAGE_LIMIT: usize = 20;
pub const DETAIL_PAGE_LIMIT: usize = 40;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliInvocation {
    pub json_mode: bool,
    pub color: ColorChoice,
    pub no_write: bool,
    pub command: CliCommand,
}

/// Explicit color selection for human-readable output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    Init(InitArgs),
    Check(CheckArgs),
    Build(BuildArgs),
    Disk(DiskArgs),
    Review,
    Sync { update: Option<String> },
    Config(ConfigArgs),
    Spec(SpecArgs),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecArgs {
    pub operation: SpecOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecOperation {
    List {
        area: Option<String>,
        feature: Option<String>,
        group: Option<String>,
    },
    Show {
        id: String,
    },
    Add {
        id: String,
        title: String,
        ears: String,
        design: Option<String>,
        manual: bool,
    },
    Set {
        id: String,
        field: String,
        value: String,
    },
    Remove {
        id: String,
    },
    Trace {
        ids: Vec<String>,
    },
    TraceReport {
        rust_cfg: Option<String>,
    },
    Candidates,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigArgs {
    pub operation: ConfigOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigOperation {
    Get { key: String },
    Set { key: String, value: String },
    Unset { key: String },
    List { prefix: Option<String> },
    Upgrade { dry_run: bool, diff: bool },
    Document,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiskArgs {
    pub tree: bool,
    pub scope: DiskScope,
    pub derivation: Option<String>,
    pub systems: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InitArgs {
    pub directory: Option<PathBuf>,
    pub template: Option<String>,
    pub force: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildArgs {
    pub packages: Vec<String>,
    pub systems: Vec<String>,
    pub check: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DiskScope {
    #[default]
    All,
    Runtime,
    Build,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckArgs {
    pub operation: Option<CheckOperation>,
    pub selectors: Vec<String>,
    pub systems: Vec<String>,
    pub jobs: Option<usize>,
    pub fail_fast: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOperation {
    List(ListArgs),
    Failures(FailureArgs),
    Details(DetailsArgs),
    Health(HealthArgs),
    HealthUpdate(HealthUpdateArgs),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthArgs {
    pub run: Option<String>,
    pub offset: usize,
    pub limit: usize,
}

impl Default for HealthArgs {
    fn default() -> Self {
        Self {
            run: None,
            offset: 0,
            limit: DEFAULT_PAGE_LIMIT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HealthUpdateArgs {
    pub run: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListArgs {
    pub selectors: Vec<String>,
    pub systems: Vec<String>,
    pub offset: usize,
    pub limit: usize,
}

impl Default for ListArgs {
    fn default() -> Self {
        Self {
            selectors: Vec::new(),
            systems: Vec::new(),
            offset: 0,
            limit: DEFAULT_PAGE_LIMIT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureArgs {
    pub run: Option<String>,
    pub offset: usize,
    pub limit: usize,
}

impl Default for FailureArgs {
    fn default() -> Self {
        Self {
            run: None,
            offset: 0,
            limit: DEFAULT_PAGE_LIMIT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailsArgs {
    pub failure: String,
    pub run: Option<String>,
    pub offset: Option<usize>,
    pub limit: usize,
}

impl CliCommand {
    /// Top-level command name as typed after `bloomery`.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Init(_) => "init",
            Self::Check(_) => "check",
            Self::Build(_) => "build",
            Self::Disk(_) => "disk",
            Self::Review => "review",
            Self::Sync { .. } => "sync",
            Self::Config(_) => "config",
            Self::Spec(_) => "spec",
        }
    }
}

impl CheckOperation {
    /// Command path of this operation, such as `check health update`.
    pub fn path(&self) -> &'static str {
        match self {
            Self::List(_) => "check list",
            Self::Failures(_) => "check failures",
            Self::Details(_) => "check details",
            Self::Health(_) => "check health",
            Self::HealthUpdate(_) => "check health update",
        }
    }
}

impl ConfigOperation {
    /// Command path of this operation, such as `config set`.
    pub fn path(&self) -> &'static str {
        match self {
            Self::Get { .. } => "config get",
            Self::Set { .. } => "config set",
            Self::Unset { .. } => "config unset",
            Self::List { .. } => "config list",
            Self::Upgrade { .. } => "config upgrade",
            Self::Document => "config document",
        }
    }
}

impl SpecOperation {
    /// Command path of this operation; trace reports share `spec trace`.
    pub fn path(&self) -> &'static str {
        match self {
            Self::List { .. } => "spec list",
            Self::Show { .. } => "spec show",
            Self::Add { .. } => "spec add",
            Self::Set { .. } => "spec set",
            Self::Remove { .. } => "spec remove",
            Self::Trace { .. } | Self::TraceReport { .. } => "spec trace",
            Self::Candidates => "spec candidates",
        }
    }
}
