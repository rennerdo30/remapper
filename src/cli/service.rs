//! Systemd service management

use anyhow::Result;
use console::style;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use super::ServiceAction;

/// User service unit file content
const SERVICE_UNIT: &str = r#"[Unit]
Description=Remapper - Linux evdev input remapping
Documentation=https://github.com/user/remapper
After=graphical-session.target

[Service]
Type=simple
ExecStart=%h/.cargo/bin/remapper run --daemon
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
"#;

/// Get the systemd user service directory
fn service_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME")?;
    Ok(PathBuf::from(home).join(".config/systemd/user"))
}

/// Get the service file path
fn service_path() -> Result<PathBuf> {
    Ok(service_dir()?.join("remapper.service"))
}

/// Handle service management commands
pub async fn handle_service(action: ServiceAction) -> Result<()> {
    match action {
        ServiceAction::Install => install_service().await,
        ServiceAction::Uninstall => uninstall_service().await,
        ServiceAction::Start => start_service().await,
        ServiceAction::Stop => stop_service().await,
        ServiceAction::Restart => restart_service().await,
        ServiceAction::Status => status_service().await,
        ServiceAction::Logs { lines, follow } => logs_service(lines, follow).await,
        ServiceAction::Enable => enable_service().await,
        ServiceAction::Disable => disable_service().await,
    }
}

/// Install the systemd user service
async fn install_service() -> Result<()> {
    let dir = service_dir()?;
    let path = service_path()?;

    // Create directory if needed
    fs::create_dir_all(&dir)?;

    // Get the actual remapper binary path
    let binary_path = std::env::current_exe()?;

    // Modify service file with actual binary path
    let service_content = SERVICE_UNIT.replace(
        "%h/.cargo/bin/remapper",
        &binary_path.to_string_lossy(),
    );

    // Write service file
    fs::write(&path, service_content)?;

    println!(
        "{}",
        style(format!("Service installed to: {}", path.display())).green()
    );

    // Reload systemd
    let status = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()?;

    if status.success() {
        println!("{}", style("Systemd configuration reloaded.").dim());
    }

    println!();
    println!("To enable the service to start on login:");
    println!("  {}", style("remapper service enable").cyan());
    println!();
    println!("To start the service now:");
    println!("  {}", style("remapper service start").cyan());

    Ok(())
}

/// Uninstall the systemd user service
async fn uninstall_service() -> Result<()> {
    let path = service_path()?;

    // Stop and disable first
    let _ = Command::new("systemctl")
        .args(["--user", "stop", "remapper.service"])
        .status();

    let _ = Command::new("systemctl")
        .args(["--user", "disable", "remapper.service"])
        .status();

    // Remove service file
    if path.exists() {
        fs::remove_file(&path)?;
        println!(
            "{}",
            style(format!("Service removed: {}", path.display())).green()
        );

        // Reload systemd
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
    } else {
        println!("{}", style("Service was not installed.").dim());
    }

    Ok(())
}

/// Start the service
async fn start_service() -> Result<()> {
    let status = Command::new("systemctl")
        .args(["--user", "start", "remapper.service"])
        .status()?;

    if status.success() {
        println!("{}", style("Service started.").green());
    } else {
        anyhow::bail!("Failed to start service");
    }

    Ok(())
}

/// Stop the service
async fn stop_service() -> Result<()> {
    let status = Command::new("systemctl")
        .args(["--user", "stop", "remapper.service"])
        .status()?;

    if status.success() {
        println!("{}", style("Service stopped.").green());
    } else {
        anyhow::bail!("Failed to stop service");
    }

    Ok(())
}

/// Restart the service
async fn restart_service() -> Result<()> {
    let status = Command::new("systemctl")
        .args(["--user", "restart", "remapper.service"])
        .status()?;

    if status.success() {
        println!("{}", style("Service restarted.").green());
    } else {
        anyhow::bail!("Failed to restart service");
    }

    Ok(())
}

/// Show service status
async fn status_service() -> Result<()> {
    let status = Command::new("systemctl")
        .args(["--user", "status", "remapper.service"])
        .status()?;

    // systemctl status returns non-zero for inactive services
    if !status.success() {
        // Still show the output, but don't fail
    }

    Ok(())
}

/// Show service logs
async fn logs_service(lines: usize, follow: bool) -> Result<()> {
    let mut args = vec!["--user", "-u", "remapper.service", "-n"];
    let lines_str = lines.to_string();
    args.push(&lines_str);

    if follow {
        args.push("-f");
    }

    Command::new("journalctl").args(&args).status()?;

    Ok(())
}

/// Enable service to start on login
async fn enable_service() -> Result<()> {
    let status = Command::new("systemctl")
        .args(["--user", "enable", "remapper.service"])
        .status()?;

    if status.success() {
        println!(
            "{}",
            style("Service enabled. It will start automatically on login.").green()
        );
    } else {
        anyhow::bail!("Failed to enable service");
    }

    Ok(())
}

/// Disable service from starting on login
async fn disable_service() -> Result<()> {
    let status = Command::new("systemctl")
        .args(["--user", "disable", "remapper.service"])
        .status()?;

    if status.success() {
        println!(
            "{}",
            style("Service disabled. It will no longer start automatically.").green()
        );
    } else {
        anyhow::bail!("Failed to disable service");
    }

    Ok(())
}
