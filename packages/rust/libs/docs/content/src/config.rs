pub struct BloomeryConfig {
    pub repo_url: &'static str,
    pub repo_name: &'static str,
    pub repo_owner: &'static str,
    pub full_repo: &'static str,
    pub default_version: &'static str,
    pub issues_url: &'static str,
    pub discussions_url: &'static str,
    pub api_repo_url: &'static str,
}

pub static CONFIG: BloomeryConfig = BloomeryConfig {
    repo_url: "https://github.com/actionable-work/bloomery",
    repo_name: "bloomery",
    repo_owner: "actionable-work",
    full_repo: "actionable-work/bloomery",
    default_version: "v0.1.0",
    issues_url: "https://github.com/actionable-work/bloomery/issues/new",
    discussions_url: "https://github.com/actionable-work/bloomery/discussions",
    api_repo_url: "https://api.github.com/repos/actionable-work/bloomery",
};
