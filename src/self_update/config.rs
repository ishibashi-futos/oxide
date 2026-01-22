#[derive(Debug, Clone)]
pub struct SelfUpdateConfig {
    pub repo: String,
    pub allow_prerelease: bool,
}

impl SelfUpdateConfig {
    pub fn new(repo: impl Into<String>, allow_prerelease: bool) -> Self {
        Self {
            repo: repo.into(),
            allow_prerelease,
        }
    }
}
