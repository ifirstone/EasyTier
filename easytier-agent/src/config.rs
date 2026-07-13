#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub server_url: String,
    pub token: Option<String>,
    pub check_interval: u64,
    pub bin_path: String,
    pub uri: String,
    pub key: String,
    pub machine_id: Option<String>,
    pub arch: Option<String>,
    pub log: bool,
    pub owner: Option<String>,
}
