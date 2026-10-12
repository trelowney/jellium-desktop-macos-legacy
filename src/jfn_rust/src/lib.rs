pub mod app;
mod app_menu;
mod cli;
pub mod manager;
mod platform_install;
pub mod shell;
mod window_geometry;

mod connection;
#[cfg(target_os = "macos")]
mod hw_decode;
