pub mod config;
pub mod download;
pub mod error;
pub mod http;
pub mod release;
pub mod replace;
pub mod service;
pub mod traits;

pub use config::SelfUpdateConfig;
pub use error::SelfUpdateError;
pub use release::{GitHubAsset, UpdateDecision, current_target_triple, current_version_tag};
pub use service::{SelfUpdatePlan, SelfUpdateService};
pub use traits::{SystemVersionEnv, VersionEnv};
