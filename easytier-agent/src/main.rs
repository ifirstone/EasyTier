use std::path::PathBuf;
use std::process::Command;

use clap::{Parser, Subcommand};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

mod config;
mod ip_monitor;
mod process;
mod web_client;

use crate::{
    config::{
        AgentConfig, default_config_path, installed_binary_path, install_dir, is_installed,
        is_running_under_systemd, service_file_path,
    },
    ip_monitor::IpMonitor,
    process::ProcessManager,
    web_client::WebClient,
};

rust_i18n::i18n!("locales", fallback = "en");

#[derive(Parser, Debug)]
#[command(name = "easytier-agent", about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long, env = "ET_AGENT_SERVER")]
    server: Option<String>,

    #[arg(short, long, env = "ET_AGENT_INTERVAL")]
    interval: Option<u64>,

    #[arg(short, long, env = "ET_AGENT_BIN")]
    bin_path: Option<String>,

    #[arg(short, long, env = "ET_AGENT_URI")]
    uri: Option<String>,

    #[arg(short, long, env = "ET_AGENT_KEY")]
    key: Option<String>,

    #[arg(short, long, env = "ET_AGENT_MACHINE_ID")]
    machine_id: Option<String>,

    #[arg(short, long, env = "ET_AGENT_ARCH")]
    arch: Option<String>,

    #[arg(long, env = "ET_AGENT_LOG")]
    log: Option<bool>,

    #[arg(long, env = "ET_AGENT_OWNER")]
    owner: Option<String>,

    #[arg(long, env = "ET_AGENT_CONFIG")]
    config: Option<PathBuf>,

    #[arg(long, env = "ET_AGENT_NO_INSTALL")]
    no_install: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Uninstall,
}

fn run_cmd(program: &str, args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        anyhow::bail!(
            "{} {:?} failed: {}",
            program,
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

async fn uninstall_agent() -> anyhow::Result<()> {
    info!("uninstalling easytier-agent");

    let _ = run_cmd("systemctl", &["stop", "easytier-agent"]);
    let _ = run_cmd("systemctl", &["disable", "easytier-agent"]);

    let service = service_file_path();
    if service.exists() {
        std::fs::remove_file(&service)?;
    }

    let installed = installed_binary_path();
    if installed.exists() {
        std::fs::remove_file(&installed)?;
    }

    let cfg = default_config_path();
    if cfg.exists() {
        std::fs::remove_file(&cfg)?;
    }

    let cfg_dir = install_dir();
    if cfg_dir.exists() {
        let _ = std::fs::remove_dir(&cfg_dir);
    }

    info!("easytier-agent uninstalled; easytier-core left running");
    Ok(())
}

async fn install_service(_config: &AgentConfig) -> anyhow::Result<()> {
    let self_path = std::env::current_exe()?;
    let target = installed_binary_path();

    if self_path != target {
        info!(source = %self_path.display(), target = %target.display(), "copying easytier-agent binary");
        std::fs::copy(&self_path, &target)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&target)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&target, perms)?;
        }
    }

    let service = format!(
        r#"[Unit]
Description=EasyTier Agent
After=network.target

[Service]
Type=simple
ExecStart={}
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
"#,
        target.display()
    );

    let svc_path = service_file_path();
    std::fs::write(&svc_path, service)?;

    run_cmd("systemctl", &["daemon-reload"])?;
    run_cmd("systemctl", &["enable", "easytier-agent"])?;
    run_cmd("systemctl", &["start", "easytier-agent"])?;

    info!("easytier-agent service installed and started");
    Ok(())
}

fn build_config(cli: &Cli) -> anyhow::Result<AgentConfig> {
    let config_path = cli
        .config
        .clone()
        .unwrap_or_else(default_config_path);

    let mut config = if config_path.exists() {
        let mut cfg = AgentConfig::load(&config_path)?;
        cfg.config_file = config_path;
        cfg
    } else {
        let mut cfg = AgentConfig::defaults();
        cfg.config_file = config_path;
        cfg
    };

    if let Some(v) = &cli.server {
        config.server_url = v.clone();
    }
    if let Some(v) = cli.interval {
        config.check_interval = v;
    }
    if let Some(v) = &cli.bin_path {
        config.bin_path = v.clone();
    }
    if let Some(v) = &cli.uri {
        config.uri = v.clone();
    }
    if let Some(v) = &cli.key {
        config.key = v.clone();
    }
    if let Some(v) = &cli.machine_id {
        config.machine_id = Some(v.clone());
    }
    if let Some(v) = &cli.arch {
        config.arch = Some(v.clone());
    }
    if let Some(v) = cli.log {
        config.log = v;
    }
    if let Some(v) = &cli.owner {
        config.owner = v.clone();
    }
    config.no_install = cli.no_install;

    config.save()?;
    Ok(config)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if let Some(Commands::Uninstall) = cli.command {
        return uninstall_agent().await;
    }

    let config = build_config(&cli)?;

    if config.log {
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
            )
            .init();
    }

    info!(
        server = %config.server_url,
        interval = %config.check_interval,
        log = %config.log,
        config_file = %config.config_file.display(),
        "starting easytier-agent"
    );

    let systemd = is_running_under_systemd();
    let installed = is_installed();
    let current_is_installed = std::env::current_exe()? == installed_binary_path();

    if !config.no_install && !systemd && !current_is_installed {
        info!("first run detected; registering and installing systemd service");
        let mut web = WebClient::new(config.clone());
        if let Err(e) = web.register().await {
            warn!(error = %e, "registration failed, continuing in foreground");
        } else {
            config.save()?;
        }
        if !installed {
            match install_service(&config).await {
                Ok(()) => {
                    info!("service installed and started; exiting foreground process");
                    return Ok(());
                }
                Err(e) => {
                    warn!(error = %e, "failed to install systemd service, continuing in foreground");
                }
            }
        } else {
            info!("service already installed; exiting foreground process");
            return Ok(());
        }
    }

    let web = WebClient::new(config.clone());
    let pm = ProcessManager::new(config.clone());
    let mut monitor = IpMonitor::new(config, web, pm);

    monitor.run().await?;
    Ok(())
}
