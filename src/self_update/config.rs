#[derive(Debug, Clone)]
pub struct SelfUpdateConfig {
    pub repo: String,
    pub allow_prerelease: bool,
    pub allow_insecure: bool,
}

impl SelfUpdateConfig {
    pub fn new(repo: impl Into<String>, allow_prerelease: bool, allow_insecure: bool) -> Self {
        Self {
            repo: repo.into(),
            allow_prerelease,
            allow_insecure,
        }
    }
}
