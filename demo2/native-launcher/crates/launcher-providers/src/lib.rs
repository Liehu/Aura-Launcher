//! Extra built-in data sources ported from demo1:
//! - `app_registry`: registry Uninstall-key application enumeration
//! - `recent_files`: Windows Recent (`%APPDATA%\Microsoft\Windows\Recent`)

pub mod app_registry;
pub mod catalog;
pub mod packaged;
pub mod icons;
pub mod recent_files;

pub use app_registry::{AppEntry, AppRegistryProvider};
pub use recent_files::RecentFilesProvider;
