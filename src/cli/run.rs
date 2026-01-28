//! Profile running functionality

use anyhow::Result;
use console::style;
use tokio::signal;
use tokio::sync::watch;
use tracing::{error, info};

use crate::config::ConfigManager;
use crate::core::RemapEngine;

/// Run profiles
pub async fn run_profiles(profile_names: Vec<String>, daemon: bool) -> Result<()> {
    let config = ConfigManager::load()?;

    // Determine which profiles to run
    let profiles_to_run: Vec<_> = if profile_names.is_empty() {
        // Run all enabled profiles
        config.enabled_profiles().into_iter().cloned().collect()
    } else {
        // Run specified profiles
        let mut profiles = Vec::new();
        for name in &profile_names {
            if let Some(profile) = config.get_profile(name) {
                profiles.push(profile.clone());
            } else {
                anyhow::bail!("Profile not found: {}", name);
            }
        }
        profiles
    };

    if profiles_to_run.is_empty() {
        println!("No profiles to run.");
        println!("Create a profile with 'remapper create' or enable existing profiles.");
        return Ok(());
    }

    if daemon {
        run_daemon(profiles_to_run).await
    } else {
        run_foreground(profiles_to_run).await
    }
}

/// Run profiles in foreground
async fn run_foreground(profiles: Vec<crate::config::Profile>) -> Result<()> {
    println!();
    println!("{}", style("Starting Remapper").bold());
    println!();

    for profile in &profiles {
        println!(
            "  {} {}",
            style("Loading:").dim(),
            style(&profile.name).cyan()
        );
    }
    println!();

    // Create shutdown signal
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Create engines for each profile
    let mut engines = Vec::new();
    for profile in profiles {
        match RemapEngine::new(&profile).await {
            Ok(engine) => {
                info!("Created engine for profile: {}", profile.name);
                engines.push((profile.name.clone(), engine));
            }
            Err(e) => {
                error!("Failed to create engine for {}: {}", profile.name, e);
                println!(
                    "  {} Failed to load {}: {}",
                    style("Error:").red(),
                    profile.name,
                    e
                );
            }
        }
    }

    if engines.is_empty() {
        anyhow::bail!("No profiles could be loaded");
    }

    // Start engines in separate tasks
    let mut handles = Vec::new();
    for (name, mut engine) in engines {
        let _shutdown = shutdown_rx.clone();
        let handle = tokio::spawn(async move {
            info!("Starting engine: {}", name);
            if let Err(e) = engine.start().await {
                error!("Engine {} failed: {}", name, e);
            }
            engine.stop().await.ok();
        });
        handles.push(handle);
    }

    println!(
        "{}",
        style("Remapper is running. Press Ctrl+C to stop.").green()
    );
    println!();

    // Wait for shutdown signal
    signal::ctrl_c().await?;

    println!();
    println!("{}", style("Shutting down...").yellow());

    // Send shutdown signal
    let _ = shutdown_tx.send(true);

    // Wait for all engines to stop
    for handle in handles {
        let _ = handle.await;
    }

    println!("{}", style("Remapper stopped.").dim());

    Ok(())
}

/// Run as a daemon
async fn run_daemon(profiles: Vec<crate::config::Profile>) -> Result<()> {
    crate::daemon::run_daemon(profiles).await
}
