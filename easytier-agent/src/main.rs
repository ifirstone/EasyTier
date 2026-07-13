use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;

mod config;
mod ip_monitor;
mod process;
mod web_client;

use crate::{config::AgentConfig, ip_monitor::IpMonitor, process::ProcessManager, web_client::WebClient};

rust_i18n::i18n!("locales", fallback = "en");

#[derive(Parser, Debug)]
#[command(name = "easytier-agent", about, long_about = None)]
struct Cli {
    #[arg(short, long, env = "ET_AGENT_SERVER", default_value = "http://localhost:11211")]
    server: String,

    #[arg(short, long, env = "ET_AGENT_TOKEN")]
    token: Option<String>,

    #[arg(short, long, env = "ET_AGENT_INTERVAL", default_value_t = 180)]
    interval: u64,

    #[arg(short, long, env = "ET_AGENT_BIN", default_value = "/usr/local/bin/easytier-core")]
    bin_path: String,

    #[arg(short, long, env = "ET_AGENT_URI")]
    uri: String,

    #[arg(short, long, env = "ET_AGENT_KEY", default_value = "sdwan")]
    key: String,

    #[arg(short, long, env = "ET_AGENT_MACHINE_ID")]
    machine_id: Option<String>,

    #[arg(short, long, env = "ET_AGENT_ARCH")]
    arch: Option<String>,

    #[arg(long, env = "ET_AGENT_LOG", default_value_t = false)]
    log: bool,

    #[arg(long, env = "ET_AGENT_OWNER")]
    owner: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = AgentConfig {
        server_url: cli.server,
        token: cli.token,
        check_interval: cli.interval,
        bin_path: cli.bin_path,
        uri: cli.uri,
        key: cli.key,
        machine_id: cli.machine_id,
        arch: cli.arch,
        log: cli.log,
        owner: cli.owner,
    };

    if config.log {
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
            )
            .init();
    }

    info!(server = %config.server_url, interval = %config.check_interval, log = %config.log, "starting easytier-agent");

    let web = WebClient::new(config.clone());
    let pm = ProcessManager::new(config.clone());
    let mut monitor = IpMonitor::new(config, web, pm);

    monitor.run().await?;
    Ok(())
}
