use std::collections::BTreeSet;
use std::fmt;

/// clap supplies this value when `--update` is used without a list.
pub const BARE_UPDATE_VALUE: &str = "__bloomery_bare_update__";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ecosystem {
    Nix,
    Rust,
}

impl Ecosystem {
    fn name(self) -> &'static str {
        match self {
            Self::Nix => "nix",
            Self::Rust => "rust",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateSelection {
    /// Reconcile locks but do not deliberately upgrade dependencies.
    None,
    /// Update every applicable ecosystem (bare `--update`).
    All,
    /// Update only the named ecosystems.
    Only(BTreeSet<Ecosystem>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageError {
    message: String,
}

impl UsageError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn exit_code(&self) -> u8 {
        2
    }
}

impl fmt::Display for UsageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for UsageError {}

/// Convert clap's optional value into the requested update policy.
pub fn parse_cli_update(value: Option<&str>) -> Result<UpdateSelection, UsageError> {
    match value {
        None => Ok(UpdateSelection::None),
        Some(BARE_UPDATE_VALUE) => Ok(UpdateSelection::All),
        Some(list) => parse_update_list(list),
    }
}

/// Parse an explicit comma-separated update list. Names are deduplicated and
/// stored in a stable order so the user's input order has no semantic effect.
pub fn parse_update_list(list: &str) -> Result<UpdateSelection, UsageError> {
    if list.is_empty() {
        return Err(UsageError::new(
            "--update requires a non-empty list when an equals sign or argument is supplied",
        ));
    }

    let mut ecosystems = BTreeSet::new();
    for value in list.split(',') {
        let value = value.trim();
        if value.is_empty() {
            return Err(UsageError::new(
                "--update does not allow empty comma-separated values",
            ));
        }
        let ecosystem = match value {
            "nix" => Ecosystem::Nix,
            "rust" => Ecosystem::Rust,
            other => {
                return Err(UsageError::new(format!(
                    "unknown --update ecosystem '{other}'; expected 'nix' or 'rust'"
                )));
            }
        };
        ecosystems.insert(ecosystem);
    }

    Ok(UpdateSelection::Only(ecosystems))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdatePlan {
    pub update_nix: bool,
    pub update_rust: bool,
}

impl UpdatePlan {
    pub fn new(selection: &UpdateSelection) -> Self {
        match selection {
            UpdateSelection::None => Self {
                update_nix: false,
                update_rust: false,
            },
            UpdateSelection::All => Self {
                update_nix: true,
                update_rust: true,
            },
            UpdateSelection::Only(ecosystems) => Self {
                update_nix: ecosystems.contains(&Ecosystem::Nix),
                update_rust: ecosystems.contains(&Ecosystem::Rust),
            },
        }
    }

    pub fn selected_names(&self) -> Vec<&'static str> {
        let mut selected = Vec::new();
        if self.update_nix {
            selected.push(Ecosystem::Nix.name());
        }
        if self.update_rust {
            selected.push(Ecosystem::Rust.name());
        }
        selected
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BARE_UPDATE_VALUE, Ecosystem, UpdatePlan, UpdateSelection, parse_cli_update,
        parse_update_list,
    };
    use bloomery_test_macros::bloomery;
    use std::collections::BTreeSet;

    #[test]
    #[bloomery("CLI-SYNC-INTERFACE-001")]
    #[bloomery("CLI-SYNC-INTERFACE-005")]
    #[bloomery("CLI-SYNC-INTERFACE-006")]
    fn update_selection_accepts_bare_and_explicit_forms() {
        assert_eq!(
            parse_cli_update(None).expect("no update flag"),
            UpdateSelection::None
        );
        assert_eq!(
            parse_cli_update(Some(BARE_UPDATE_VALUE)).expect("bare update flag"),
            UpdateSelection::All
        );
        assert_eq!(
            parse_update_list("rust,nix").expect("equals-form list"),
            UpdateSelection::Only(BTreeSet::from([Ecosystem::Nix, Ecosystem::Rust]))
        );
    }

    #[test]
    #[bloomery("CLI-SYNC-INTERFACE-002")]
    #[bloomery("CLI-SYNC-INTERFACE-011")]
    fn duplicate_names_are_deduplicated_and_order_is_irrelevant() {
        let forward = parse_update_list("rust,nix,rust").expect("forward list");
        let reverse = parse_update_list("nix,rust").expect("reverse list");
        assert_eq!(forward, reverse);
        assert_eq!(UpdatePlan::new(&forward).selected_names(), ["nix", "rust"]);
    }

    #[test]
    #[bloomery("CLI-SYNC-INTERFACE-003")]
    #[bloomery("CLI-SYNC-INTERFACE-007")]
    #[bloomery("CLI-SYNC-INTERFACE-008")]
    #[bloomery("CLI-SYNC-INTERFACE-009")]
    fn update_plans_restrict_explicit_updates_and_expand_bare_updates() {
        let reconcile = UpdatePlan::new(&UpdateSelection::None);
        assert!(!reconcile.update_nix && !reconcile.update_rust);

        let bare = UpdatePlan::new(&UpdateSelection::All);
        assert!(bare.update_nix && bare.update_rust);
        assert_eq!(bare.selected_names(), ["nix", "rust"]);

        let rust_only = parse_update_list("rust").expect("rust-only list");
        let rust_plan = UpdatePlan::new(&rust_only);
        assert!(!rust_plan.update_nix && rust_plan.update_rust);
    }

    #[test]
    #[bloomery("CLI-SYNC-INTERFACE-012")]
    #[bloomery("CLI-SYNC-INTERFACE-013")]
    #[bloomery("CLI-SYNC-INTERFACE-014")]
    fn invalid_update_lists_are_usage_errors() {
        for invalid in ["", "unknown", "rust,,nix", ",rust", "nix,"] {
            let error = parse_update_list(invalid).expect_err("invalid update list");
            assert_eq!(error.exit_code(), 2);
        }
    }
}
