#[derive(Debug, Default)]
pub struct SystemVersionEnv;

pub trait VersionEnv {
    fn get(&self, key: &str) -> Option<String>;
}

impl VersionEnv for SystemVersionEnv {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}
