use std::process::Command;

use anyhow::Result;
use tracing::{info, warn};

use crate::web_client::AgentCommand;

pub struct IpMonitor {
    config: crate::AgentConfig,
    web: crate::WebClient,
    pm: crate::ProcessManager,
    current_pid: Option<u32>,
    last_ip: String,
}

impl IpMonitor {
    pub fn new(config: crate::AgentConfig, web: crate::WebClient, pm: crate::ProcessManager) -> Self {
        Self {
            config,
            web,
            pm,
            current_pid: None,
            last_ip: String::new(),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            match self.detect_network() {
                Ok((ip, _mask, hostname)) => {
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
                    let core_status = self.current_pid.map(|pid| {
                        serde_json::json!({
                            "installed": true,
                            "pid": pid,
                            "uri": self.config.uri,
                        })
                    });
                    let _ = self.web.heartbeat(&ip, core_status).await;
                }
                Err(e) => {
                    warn!(error = %e, "network check failed");
                }
            }

            match self.web.poll_commands().await {
                Ok(commands) => {
                    for cmd in commands {
                        self.handle_command(cmd).await;
                    }
                }
                Err(e) => warn!(error = %e, "poll commands failed"),
            }

            tokio::time::sleep(std::time::Duration::from_secs(self.config.check_interval)).await;
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
            }
            "install" => {
                let version = cmd.payload.get("version").and_then(|v| v.as_str());
                warn!(version = ?version, "install command received; downloading/upgrading not implemented in agent yet");
            }
            _ => {}
        }
        let _ = self.web.ack_command(&cmd.id).await;
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
