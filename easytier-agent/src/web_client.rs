use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

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
    agent_id: String,
    token: String,
}

impl WebClient {
    pub fn new(config: AgentConfig) -> Self {
        let (agent_id, token) = if let Some(t) = config.token.clone() {
            (format!("agent-{}", Uuid::new_v4().simple()), t)
        } else {
            (String::new(), String::new())
        };
        Self {
            config,
            client: Client::builder()
                .user_agent("easytier-agent/1.0")
                .build()
                .unwrap_or_else(|_| Client::new()),
            agent_id,
            token,
        }
    }

    async fn ensure_registered(&mut self) -> anyhow::Result<()> {
        if !self.token.is_empty() {
            return Ok(());
        }
        let hostname = hostname::get()?.to_string_lossy().to_string();
        let os = std::env::consts::OS.to_string();
        let arch = self.config.arch.clone().unwrap_or_else(|| std::env::consts::ARCH.to_string());
        let owner = self.config.owner.clone().unwrap_or_else(|| "default".to_string());
        let req = serde_json::json!({
            "hostname": hostname,
            "public_ip": "",
            "arch": arch,
            "os": os,
            "current_version": None::<String>,
            "owner": owner,
        });
        let url = format!("{}/api/v1/agent/register", self.config.server_url);
        let resp = self.client.post(&url).json(&req).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("register failed: {}", resp.status());
        }
        let data: serde_json::Value = resp.json().await?;
        if let Some(id) = data.get("agent_id").and_then(|v| v.as_str()) {
            self.agent_id = id.to_string();
        }
        if let Some(t) = data.get("token").and_then(|v| v.as_str()) {
            self.token = t.to_string();
            info!(agent_id = %self.agent_id, token = %self.token, "registered with server");
        }
        Ok(())
    }

    pub async fn heartbeat(&mut self, ip: &str, core_status: Option<serde_json::Value>) -> anyhow::Result<()> {
        self.ensure_registered().await?;
        if self.token.is_empty() {
            return Ok(());
        }
        let req = serde_json::json!({
            "agent_id": self.agent_id,
            "token": self.token,
            "public_ip": ip,
            "current_version": None::<String>,
            "status": "running",
            "message": None::<String>,
            "core_status": core_status,
        });
        let url = format!("{}/api/v1/agent/heartbeat", self.config.server_url);
        let resp = self.client.post(&url).json(&req).send().await?;
        if !resp.status().is_success() {
            warn!(status = %resp.status(), "heartbeat failed");
        }
        Ok(())
    }

    pub async fn poll_commands(&mut self) -> anyhow::Result<Vec<AgentCommand>> {
        self.ensure_registered().await?;
        if self.agent_id.is_empty() {
            return Ok(vec![]);
        }
        let url = format!("{}/api/v1/agent/commands/{}", self.config.server_url, self.agent_id);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Ok(vec![]);
        }
        let data: AgentCommandsResponse = resp.json().await?;
        Ok(data.commands)
    }

    pub async fn ack_command(&self, command_id: &str) -> anyhow::Result<()> {
        if self.agent_id.is_empty() {
            return Ok(());
        }
        let req = serde_json::json!({
            "agent_id": self.agent_id,
            "token": self.token,
            "command_id": command_id,
            "result": {},
        });
        let url = format!("{}/api/v1/agent/commands/ack", self.config.server_url);
        let _ = self.client.post(&url).json(&req).send().await?;
        Ok(())
    }
}
