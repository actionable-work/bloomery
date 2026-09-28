const ASSET_CONTENT: &str = include_str!("../assets/data.txt");
const ENV_VAR: &str = env!("BLOOMERY_TEST_VAR");
const TOP_LEVEL_VAR: &str = env!("TOP_LEVEL_OVERRIDE_VAR");

#[cfg(bloomery_overrides_validated)]
const CFG_ACTIVE: bool = true;

#[cfg(not(bloomery_overrides_validated))]
const CFG_ACTIVE: bool = false;

#[cfg(bloomery_toplevel_overrides_validated)]
const TOP_LEVEL_CFG_ACTIVE: bool = true;

#[cfg(not(bloomery_toplevel_overrides_validated))]
const TOP_LEVEL_CFG_ACTIVE: bool = false;

fn main() {
    println!("Asset: {}", ASSET_CONTENT.trim());
    println!("Env: {}", ENV_VAR);
    println!("TopLevelEnv: {}", TOP_LEVEL_VAR);
    println!("Cfg: {}", CFG_ACTIVE);
    println!("TopLevelCfg: {}", TOP_LEVEL_CFG_ACTIVE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fileset_inclusion() {
        assert_eq!(ASSET_CONTENT.trim(), "hello-from-fileset-asset");
    }

    #[test]
    fn test_env_override() {
        assert_eq!(ENV_VAR, "injected_from_overrides_nix");
    }

    #[test]
    fn test_cfg_override() {
        assert!(
            CFG_ACTIVE,
            "Expected bloomery_overrides_validated cfg to be enabled!"
        );
    }

    #[test]
    fn test_toplevel_env_override() {
        assert_eq!(TOP_LEVEL_VAR, "injected_from_flake_nix");
    }

    #[test]
    fn test_toplevel_cfg_override() {
        assert!(
            TOP_LEVEL_CFG_ACTIVE,
            "Expected bloomery_toplevel_overrides_validated cfg to be enabled!"
        );
    }

    #[test]
    fn test_runtime_asset_file_presence() {
        // Assets are available either at ./assets (current dir during tests) or adjacent to exe
        let path = if std::path::Path::new("assets/data.txt").is_file() {
            std::path::PathBuf::from("assets/data.txt")
        } else if let Ok(exe) = std::env::current_exe() {
            exe.parent().unwrap().join("assets").join("data.txt")
        } else {
            panic!("Could not locate runtime assets/data.txt");
        };
        assert!(path.is_file(), "assets/data.txt must exist at runtime");
        let content = std::fs::read_to_string(path).expect("read runtime asset file");
        assert_eq!(content.trim(), "hello-from-fileset-asset");
    }
}
