//! Graphical user interface using iced

mod app;
mod device_picker;
mod event_viewer;
mod mapping_editor;
mod profile_editor;
mod theme;

pub use app::RemapperApp;

use anyhow::Result;

/// Run the GUI application
pub async fn run_gui() -> Result<()> {
    iced::application(RemapperApp::title, RemapperApp::update, RemapperApp::view)
        .theme(RemapperApp::theme)
        .window_size(iced::Size::new(900.0, 600.0))
        .run_with(RemapperApp::new)?;

    Ok(())
}
