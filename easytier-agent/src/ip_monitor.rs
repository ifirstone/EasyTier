use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::Result;
use tracing::{info, warn};

use crate::web_client::AgentCommand;

pub struct IpMonitor {
    config: crate::AgentConfig,
    web: crate::WebClient,
    pm: crate::ProcessManager,
    current_pid: Option<u32>,
    last_ip: String,
    last_hostname: String,
}

impl IpMonitor {
    pub fn new(config: crate::AgentConfig, web: crate::WebClient, pm: crate::ProcessManager) -> Self {
        Self {
            config,
            web,
            pm,
            current_pid: None,
            last_ip: String::new(),
            last_hostname: String::new(),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut last_ip_check = Instant::now() - Duration::from_secs(self.config.check_interval);
        loop {
            let now = Instant::now();
            if now.duration_since(last_ip_check) >= Duration::from_secs(self.config.check_interval) {
                self.check_ip_and_heartbeat().await;
                last_ip_check = now;
            }

            match self.web.poll_commands().await {
                Ok(commands) => {
                    for cmd in commands {
                        self.handle_command(cmd).await;
                    }
                }
                Err(e) => warn!(error = %e, "poll commands failed"),
            }

            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    async fn check_ip_and_heartbeat(&mut self) {
        match self.detect_network() {
            Ok((ip, _mask, hostname)) => {
                self.last_hostname = hostname.clone();
                if ip != self.last_ip {
                    info!(old = %self.last_ip, new = %ip, "ip changed");
                    if let Some(pid) = self.current_pid {
                        let _ = self.pm.stop(pid);
                    }
                    match self.pm.start(&hostname) {
                        Ok(Some(pid)) => {
                            self.current_pid = Some(pid);
                            self.last_ip = ip.clone();
                        }
                        Ok(None) => {}
                        Err(e) => warn!(error = %e, "failed to start core"),
                    }
                }
                let core_status = serde_json::json!({
                    "installed": self.is_core_installed(),
                    "running": self.current_pid.is_some(),
                    "pid": self.current_pid,
                    "uri": self.config.uri,
                });
                let _ = self.web.heartbeat(&ip, Some(core_status)).await;
            }
            Err(e) => {
                warn!(error = %e, "network check failed");
            }
        }
    }

    async fn handle_command(&mut self, cmd: AgentCommand) {
        info!(command_id = %cmd.id, command_type = %cmd.command_type, "handling command");
        match cmd.command_type.as_str() {
            "restart" => {
                if let Some(pid) = self.current_pid {
                    let _ = self.pm.stop(pid);
                    self.current_pid = None;
                }
                if !self.last_hostname.is_empty() {
                    match self.pm.start(&self.last_hostname) {
                        Ok(Some(pid)) => {
                            self.current_pid = Some(pid);
                            info!(pid = %pid, "core restarted");
                        }
                        Ok(None) => {}
                        Err(e) => warn!(error = %e, "failed to restart core"),
                    }
                }
            }
            "install" => {
                let version = cmd.payload.get("version").and_then(|v| v.as_str()).map(|s| s.to_string());
                if let Err(e) = self.install_core(version.as_deref()).await {
                    warn!(error = %e, "install core failed");
                }
            }
            "uninstall" => {
                if let Some(pid) = self.current_pid {
                    let _ = self.pm.stop(pid);
                    self.current_pid = None;
                }
                let keep_config = cmd.payload.get("keep_config").and_then(|v| v.as_bool()).unwrap_or(false);
                if let Err(e) = self.uninstall_core(keep_config).await {
                    warn!(error = %e, "uninstall core failed");
                }
            }
            "stop" => {
                if let Some(pid) = self.current_pid {
                    let _ = self.pm.stop(pid);
                    self.current_pid = None;
                    info!(pid = %pid, "core stopped");
                }
            }
            _ => {}
        }
        let _ = self.web.ack_command(&cmd.id).await;
    }

    fn is_core_installed(&self) -> bool {
        Path::new(&self.config.bin_path).is_file()
    }

    async fn install_core(&mut self, version: Option<&str>) -> Result<()> {
        let arch = self.config.arch.clone().unwrap_or_else(|| std::env::consts::ARCH.to_string());
        let mut version = match version {
            Some(v) if !v.is_empty() => v.to_string(),
            _ => {
                let latest = reqwest::get("https://api.github.com/repos/EasyTier/EasyTier/releases/latest")
                    .await?
                    .error_for_status()?
                    .json::<serde_json::Value>()
                    .await?;
                latest["tag_name"].as_str().unwrap_or("v2.6.4").to_string()
            }
        };
        if !version.starts_with('v') {
            version = format!("v{}", version);
        }

        let url = format!(
            "https://github.com/EasyTier/EasyTier/releases/download/{}/easytier-linux-{}-{}.zip",
            version, arch, version
        );
        info!(version = %version, url = %url, "downloading easytier-core");

        let bytes = reqwest::get(&url).await?.error_for_status()?.bytes().await?;
        info!(size = bytes.len(), "download complete");

        let target_dir = Path::new(&self.config.bin_path)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("/usr/local/bin"));
        let temp_dir = PathBuf::from(format!("/tmp/easytier-agent-install-{}-{}-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
            version, arch, rand::random::<u32>()
        ));
        std::fs::create_dir_all(&temp_dir)?;

        let mut archive = zip::ZipArchive::new(Cursor::new(bytes.to_vec()))?;
        archive.extract(&temp_dir)?;

        let extracted_dir = temp_dir.join(format!("easytier-linux-{}", arch));
        let source = if extracted_dir.join("easytier-core").is_file() {
            extracted_dir.join("easytier-core")
        } else {
            let mut found = None;
            for entry in walkdir::WalkDir::new(&temp_dir).max_depth(3) {
                let entry = entry?;
                if entry.file_name() == "easytier-core" {
                    found = Some(entry.path().to_path_buf());
                    break;
                }
            }
            found.ok_or_else(|| anyhow::anyhow!("easytier-core not found in downloaded archive"))?
        };

        std::fs::create_dir_all(&target_dir)?;
        let target = PathBuf::from(&self.config.bin_path);
        info!(source = %source.display(), target = %target.display(), "installing easytier-core");
        std::fs::copy(&source, &target)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&target)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&target, perms)?;
        }
        std::fs::remove_dir_all(&temp_dir)?;

        info!("easytier-core installed successfully");
        if self.current_pid.is_some() {
            info!("restarting core with new binary");
            if let Some(pid) = self.current_pid {
                let _ = self.pm.stop(pid);
                self.current_pid = None;
            }
            if !self.last_hostname.is_empty() {
                match self.pm.start(&self.last_hostname) {
                    Ok(Some(pid)) => {
                        self.current_pid = Some(pid);
                        info!(pid = %pid, "core restarted with new binary");
                    }
                    Ok(None) => {}
                    Err(e) => warn!(error = %e, "failed to restart core after install"),
                }
            }
        }
        Ok(())
    }

    async fn uninstall_core(&mut self, keep_config: bool) -> Result<()> {
        let bin = PathBuf::from(&self.config.bin_path);
        if bin.is_file() {
            std::fs::remove_file(&bin)?;
        }
        if !keep_config {
            let cli_bin = bin.parent().map(|p| p.join("easytier-cli"));
            if let Some(cli) = cli_bin {
                let _ = std::fs::remove_file(&cli);
            }
        }
        info!(keep_config = keep_config, "easytier-core uninstalled");
        Ok(())
    }

    fn detect_network(&self) -> Result<(String, String, String)> {
        let output = Command::new("ip")
            .args(["-4", "addr", "show"])
            .output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let ip = stdout
            .lines()
            .find_map(|line| {
                let line = line.trim();
                if line.starts_with("inet ") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let addr = parts[1].split('/').next().unwrap_or("");
                        if addr != "127.0.0.1" && !addr.is_empty() {
                            return Some(addr.to_string());
                        }
                    }
                }
                None
            })
            .ok_or_else(|| anyhow::anyhow!("no ip found"))?;

        let mask = stdout
            .lines()
            .find_map(|line| {
                let line = line.trim();
                if line.starts_with("inet ") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        return parts[1].split('/').nth(1);
                    }
                }
                None
            })
            .unwrap_or("24");

        let hostname = hostname::get()?.to_string_lossy().to_string();
        let suffix = self.encrypt(&ip, mask, &self.config.key);
        let calculated = format!("{}-{}", hostname, suffix);
        Ok((ip, mask.to_string(), calculated))
    }

    fn encrypt(&self, ip: &str, mask: &str, key: &str) -> String {
        let mut parts: Vec<u8> = ip.split('.').filter_map(|p| p.parse().ok()).collect();
        if let Ok(m) = mask.parse::<u8>() {
            parts.push(m);
        }
        let key_bytes = key.as_bytes();
        let mut hex = String::new();
        for (i, val) in parts.iter().enumerate() {
            let k = key_bytes[i % key_bytes.len()];
            let x = val ^ k;
            hex.push_str(&format!("{:02X}", x));
        }
        hex
    }
}
