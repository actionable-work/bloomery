use std::path::PathBuf;

pub const DEFAULT_PAGE_LIMIT: usize = 20;
pub const DETAIL_PAGE_LIMIT: usize = 40;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliInvocation {
    pub json_mode: bool,
    pub command: CliCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    Init(InitArgs),
    Check(CheckArgs),
    Review,
    Sync { update: Option<String> },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InitArgs {
    pub directory: Option<PathBuf>,
    pub template: Option<String>,
    pub force: bool,
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
