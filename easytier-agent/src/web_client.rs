use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use super::config::AgentConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCommand {
    pub id: String,
    pub command_type: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCommandsResponse {
    pub commands: Vec<AgentCommand>,
}

pub struct WebClient {
    config: AgentConfig,
    client: Client,
}

impl WebClient {
    pub fn new(config: AgentConfig) -> Self {
        Self {
            config,
            client: Client::builder()
                .user_agent("easytier-agent/1.0")
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    pub async fn register(&mut self) -> anyhow::Result<()> {
        self.ensure_registered().await
    }

    async fn ensure_registered(&mut self) -> anyhow::Result<()> {
        if !self.config.token.is_empty() && !self.config.agent_id.is_empty() {
            return Ok(());
        }
        let hostname = hostname::get()?.to_string_lossy().to_string();
        let os = std::env::consts::OS.to_string();
        let arch = self.config.arch.clone().unwrap_or_else(|| std::env::consts::ARCH.to_string());
        let owner = self.config.owner.clone();
        let req = serde_json::json!({
            "hostname": hostname,
            "public_ip": "",
            "arch": arch,
            "os": os,
            "current_version": None::<String>,
            "owner": owner,
        });
        let url = format!("{}/api/v1/agent/register", self.config.server_url);
        tracing::debug!(url = %url, body = %req, "sending register request");
        let resp = self.client.post(&url).json(&req).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("register failed: {}", resp.status());
        }
        let data: serde_json::Value = resp.json().await?;
        tracing::debug!(response = %data, "register response");
        if let Some(id) = data.get("agent_id").and_then(|v| v.as_str()) {
            self.config.agent_id = id.to_string();
        }
        if let Some(t) = data.get("token").and_then(|v| v.as_str()) {
            self.config.token = t.to_string();
            info!(agent_id = %self.config.agent_id, token = %self.config.token, "registered with server");
            let _ = self.config.save();
        }
        Ok(())
    }

    pub async fn heartbeat(&mut self, ip: &str, core_status: Option<serde_json::Value>) -> anyhow::Result<()> {
        self.ensure_registered().await?;
        if self.config.token.is_empty() {
            return Ok(());
        }
        let req = serde_json::json!({
            "agent_id": self.config.agent_id,
            "token": self.config.token,
            "public_ip": ip,
            "current_version": None::<String>,
            "status": "running",
            "message": None::<String>,
            "core_status": core_status,
        });
        let url = format!("{}/api/v1/agent/heartbeat", self.config.server_url);
        tracing::debug!(url = %url, body = %req, "sending heartbeat request");
        let resp = self.client.post(&url).json(&req).send().await?;
        if !resp.status().is_success() {
            warn!(status = %resp.status(), "heartbeat failed");
        } else {
            let data: serde_json::Value = resp.json().await?;
            tracing::debug!(response = %data, "heartbeat response");
        }
        Ok(())
    }

    pub async fn poll_commands(&mut self) -> anyhow::Result<Vec<AgentCommand>> {
        self.ensure_registered().await?;
        if self.config.agent_id.is_empty() {
            return Ok(vec![]);
        }
        let url = format!("{}/api/v1/agent/commands/{}", self.config.server_url, self.config.agent_id);
        tracing::debug!(url = %url, "fetching commands");
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Ok(vec![]);
        }
        let data: AgentCommandsResponse = resp.json().await?;
        tracing::debug!(command_count = data.commands.len(), "commands response");
        Ok(data.commands)
    }

    pub async fn ack_command(&self, command_id: &str) -> anyhow::Result<()> {
        if self.config.agent_id.is_empty() {
            return Ok(());
        }
        let req = serde_json::json!({
            "agent_id": self.config.agent_id,
            "token": self.config.token,
            "command_id": command_id,
            "result": {},
        });
        let url = format!("{}/api/v1/agent/commands/ack", self.config.server_url);
        tracing::debug!(url = %url, body = %req, "acking command");
        let _ = self.client.post(&url).json(&req).send().await?;
        Ok(())
    }
}
