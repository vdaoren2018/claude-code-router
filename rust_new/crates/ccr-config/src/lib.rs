//! 配置服务入口。

mod error;
mod interpolate;
mod service;

pub use error::ConfigError;
pub use interpolate::interpolate_env_vars;
pub use service::{AppConfig, ConfigOptions, ConfigService};
