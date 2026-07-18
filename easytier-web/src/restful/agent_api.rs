use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

use super::AppStateInner;
use super::users::AuthSession;

const AGENT_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegisterRequest {
    pub hostname: String,
    pub public_ip: String,
    pub arch: String,
    pub os: String,
    pub current_version: Option<String>,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegisterResponse {
    pub agent_id: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHeartbeatRequest {
    pub agent_id: String,
    pub token: String,
    pub public_ip: String,
    pub current_version: Option<String>,
    pub status: String,
    pub message: Option<String>,
    pub core_status: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHeartbeatResponse {
    pub ok: bool,
    pub message: Option<String>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCommandAckRequest {
    pub agent_id: String,
    pub token: String,
    pub command_id: String,
    pub result: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCommandAckResponse {
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub agent_id: String,
    pub owner: String,
    pub hostname: String,
    pub public_ip: String,
    pub arch: String,
    pub os: String,
    pub current_version: Option<String>,
    pub status: String,
    pub message: Option<String>,
    pub core_installed: bool,
    pub core_version: Option<String>,
    pub core_running: bool,
    pub core_uri: Option<String>,
    pub last_heartbeat: String,
    pub registered_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentListResponse {
    pub agents: Vec<AgentInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCommandRequest {
    pub command_type: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCommandResponse {
    pub command_id: String,
    pub command_type: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallCommandPayload {
    pub version: Option<String>,
    pub proxy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UninstallCommandPayload {
    pub keep_config: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartCommandPayload {
    pub uri: Option<String>,
    pub hostname: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopCommandPayload {}

#[derive(Debug, Clone)]
struct AgentRecord {
    info: AgentInfo,
    token: String,
    commands: Vec<AgentCommand>,
    updated_at: SystemTime,
}

impl AgentRecord {
    fn new(req: AgentRegisterRequest, owner: String) -> Self {
        let now = chrono::Utc::now();
        let agent_id = format!("agent-{}", Uuid::new_v4().simple());
        let token = Uuid::new_v4().simple().to_string();
        let info = AgentInfo {
            agent_id: agent_id.clone(),
            owner,
            hostname: req.hostname,
            public_ip: req.public_ip,
            arch: req.arch,
            os: req.os,
            current_version: req.current_version,
            status: "online".to_string(),
            message: None,
            core_installed: false,
            core_version: None,
            core_running: false,
            core_uri: None,
            last_heartbeat: now.to_rfc3339(),
            registered_at: now.to_rfc3339(),
        };
        Self {
            info,
            token,
            commands: vec![],
            updated_at: SystemTime::now(),
        }
    }

    fn touch(&mut self, req: &AgentHeartbeatRequest) {
        self.info.public_ip = req.public_ip.clone();
        self.info.current_version = req.current_version.clone();
        self.info.status = req.status.clone();
        self.info.message = req.message.clone();
        self.info.last_heartbeat = chrono::Utc::now().to_rfc3339();
        if let Some(core) = req.core_status.as_ref() {
            if let Some(installed) = core.get("installed").and_then(|v| v.as_bool()) {
                self.info.core_installed = installed;
            }
            if let Some(running) = core.get("running").and_then(|v| v.as_bool()) {
                self.info.core_running = running;
            } else if let Some(pid) = core.get("pid").and_then(|v| v.as_u64()) {
                self.info.core_running = self.is_pid_running(pid as u32);
            }
            if let Some(uri) = core.get("uri").and_then(|v| v.as_str()) {
                self.info.core_uri = Some(uri.to_string());
            }
            if let Some(version) = core.get("version").and_then(|v| v.as_str()) {
                self.info.core_version = Some(version.to_string());
            }
        }
        self.updated_at = SystemTime::now();
    }

    fn is_pid_running(&self, pid: u32) -> bool {
        #[cfg(target_os = "linux")]
        {
            std::path::Path::new(&format!("/proc/{}", pid)).exists()
        }
        #[cfg(not(target_os = "linux"))]
        {
            true
        }
    }

    fn expired(&self) -> bool {
        SystemTime::now().duration_since(self.updated_at).unwrap_or_default() > AGENT_TIMEOUT
    }
}

#[derive(Default)]
pub struct AgentStore {
    owners: DashMap<String, DashMap<String, AgentRecord>>,
}

impl AgentStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for AgentStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentStore").finish()
    }
}

impl AgentStore {
    pub fn register(&self, owner: String, req: AgentRegisterRequest) -> (String, String) {
        if let Some(owner_map) = self.owners.get(&owner) {
            for entry in owner_map.iter() {
                let record = entry.value();
                if record.info.hostname == req.hostname {
                    return (entry.key().clone(), record.token.clone());
                }
            }
        }
        let record = AgentRecord::new(req, owner.clone());
        let agent_id = record.info.agent_id.clone();
        let token = record.token.clone();
        let owner_map = self.owners.entry(owner).or_default();
        owner_map.insert(agent_id.clone(), record);
        (agent_id, token)
    }

    pub fn heartbeat(&self, agent_id: &str, token: &str, req: AgentHeartbeatRequest) -> bool {
        for owner_map in self.owners.iter() {
            if let Some(mut record) = owner_map.get_mut(agent_id) {
                if record.token != token {
                    return false;
                }
                record.touch(&req);
                if record.info.status != "online" {
                    record.info.status = "online".to_string();
                    record.info.message = None;
                }
                return true;
            }
        }
        false
    }

    pub fn get(&self, agent_id: &str) -> Option<AgentInfo> {
        for owner_map in self.owners.iter() {
            if let Some(record) = owner_map.get(agent_id) {
                return Some(record.info.clone());
            }
        }
        None
    }

    pub fn list_by_owner(&self, owner: &str) -> Vec<AgentInfo> {
        if let Some(owner_map) = self.owners.get(owner) {
            let mut agents: Vec<_> = owner_map.iter().map(|r| r.info.clone()).collect();
            for agent in &mut agents {
                if let Some(record) = owner_map.get(&agent.agent_id) {
                    if record.expired() {
                        agent.status = "offline".to_string();
                        agent.message = Some("heartbeat timeout".to_string());
                    }
                }
            }
            agents
        } else {
            vec![]
        }
    }

    pub fn push_command(&self, agent_id: &str, command: AgentCommand) -> bool {
        for owner_map in self.owners.iter() {
            if let Some(mut record) = owner_map.get_mut(agent_id) {
                record.commands.push(command);
                return true;
            }
        }
        false
    }

    pub fn pop_commands(&self, agent_id: &str) -> Vec<AgentCommand> {
        for owner_map in self.owners.iter() {
            if let Some(mut record) = owner_map.get_mut(agent_id) {
                let cmds = record.commands.clone();
                record.commands.clear();
                return cmds;
            }
        }
        vec![]
    }

    pub fn delete(&self, owner: &str, agent_id: &str) -> bool {
        if let Some(owner_map) = self.owners.get_mut(owner) {
            owner_map.remove(agent_id).is_some()
        } else {
            false
        }
    }

    pub fn update_core_status(&self, agent_id: &str, installed: bool, version: Option<String>, running: bool, uri: Option<String>) {
        for owner_map in self.owners.iter() {
            if let Some(mut record) = owner_map.get_mut(agent_id) {
                record.info.core_installed = installed;
                record.info.core_version = version;
                record.info.core_running = running;
                record.info.core_uri = uri;
                return;
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct AgentManager {
    pub store: Arc<AgentStore>,
}

impl AgentManager {
    pub fn new() -> Self {
        Self {
            store: Arc::new(AgentStore::new()),
        }
    }
}

#[instrument(skip(_state))]
async fn handle_register(
    State(_state): State<AppStateInner>,
    Json(req): Json<AgentRegisterRequest>,
) -> Result<Json<AgentRegisterResponse>, StatusCode> {
    let owner = req.owner.clone().unwrap_or_else(|| "default".to_string());
    let (agent_id, token) = _state.agent_manager.store.register(owner, req.clone());
    tracing::info!(agent_id = %agent_id, hostname = %req.hostname, "agent registered");
    Ok(Json(AgentRegisterResponse { agent_id, token }))
}

#[instrument(skip(_state))]
async fn handle_heartbeat(
    State(_state): State<AppStateInner>,
    Json(req): Json<AgentHeartbeatRequest>,
) -> Result<Json<AgentHeartbeatResponse>, StatusCode> {
    let ok = _state.agent_manager.store.heartbeat(&req.agent_id, &req.token, req.clone());
    tracing::debug!(agent_id = %req.agent_id, ip = %req.public_ip, status = %req.status, ok = %ok, "agent heartbeat");
    Ok(Json(AgentHeartbeatResponse {
        ok,
        message: if ok { None } else { Some("invalid agent_id or token".to_string()) },
    }))
}

#[instrument(skip(_state))]
async fn handle_get_commands(
    State(_state): State<AppStateInner>,
    Path(agent_id): Path<String>,
) -> Result<Json<AgentCommandsResponse>, StatusCode> {
    let commands = _state.agent_manager.store.pop_commands(&agent_id);
    tracing::debug!(agent_id = %agent_id, command_count = commands.len(), "agent get commands");
    Ok(Json(AgentCommandsResponse { commands }))
}

#[instrument(skip(_state))]
async fn handle_ack_command(
    State(_state): State<AppStateInner>,
    Json(req): Json<AgentCommandAckRequest>,
) -> Result<Json<AgentCommandAckResponse>, StatusCode> {
    tracing::info!(agent_id = %req.agent_id, command_id = %req.command_id, "agent ack command");
    Ok(Json(AgentCommandAckResponse { ok: true }))
}

#[instrument(skip(_state, auth_session))]
async fn handle_list_agents(
    auth_session: AuthSession,
    State(_state): State<AppStateInner>,
) -> Result<Json<AgentListResponse>, StatusCode> {
    let username = auth_session
        .user
        .as_ref()
        .map(|u| u.db_user.username.clone())
        .unwrap_or_default();
    let agents = _state.agent_manager.store.list_by_owner(&username);
    Ok(Json(AgentListResponse { agents }))
}

#[instrument(skip(_state, auth_session))]
async fn handle_delete_agent(
    auth_session: AuthSession,
    State(_state): State<AppStateInner>,
    Path(agent_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let username = auth_session
        .user
        .as_ref()
        .map(|u| u.db_user.username.clone())
        .unwrap_or_default();
    let ok = _state.agent_manager.store.delete(&username, &agent_id);
    if ok {
        tracing::info!(agent_id = %agent_id, "agent deleted");
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[instrument(skip(_state, auth_session))]
async fn handle_send_command(
    auth_session: AuthSession,
    State(_state): State<AppStateInner>,
    Path(agent_id): Path<String>,
    Json(req): Json<AgentCommandRequest>,
) -> Result<Json<AgentCommandResponse>, StatusCode> {
    let _username = auth_session
        .user
        .as_ref()
        .map(|u| u.db_user.username.clone())
        .unwrap_or_default();
    let command = AgentCommand {
        id: Uuid::new_v4().simple().to_string(),
        command_type: req.command_type.clone(),
        payload: req.payload.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let ok = _state.agent_manager.store.push_command(&agent_id, command.clone());
    if !ok {
        return Err(StatusCode::NOT_FOUND);
    }
    tracing::info!(agent_id = %agent_id, command_id = %command.id, command_type = %command.command_type, "send agent command");
    Ok(Json(AgentCommandResponse {
        command_id: command.id,
        command_type: command.command_type,
        payload: command.payload,
        created_at: command.created_at,
    }))
}

pub fn build_public_route() -> Router<AppStateInner> {
    Router::new()
        .route("/api/v1/agent/register", post(handle_register))
        .route("/api/v1/agent/heartbeat", post(handle_heartbeat))
        .route("/api/v1/agent/commands/:agent_id", get(handle_get_commands))
        .route("/api/v1/agent/commands/ack", post(handle_ack_command))
}

pub fn build_management_route() -> Router<AppStateInner> {
    Router::new()
        .route("/api/v1/agent", get(handle_list_agents))
        .route("/api/v1/agent/:agent_id", delete(handle_delete_agent))
        .route("/api/v1/agent/:agent_id/commands", post(handle_send_command))
}
