use crate::auth::OAuthConfig;
use crate::db::Db;
use crate::youtube::YtClient;
use std::sync::Arc;

pub struct AppState {
    pub db: Arc<Db>,
    pub http: reqwest::Client,
    pub api_key: Option<String>,
    pub oauth: Option<OAuthConfig>,
}

impl AppState {
    pub fn new() -> Self {
        let data_dir = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join("yt-client");
        let db_path = data_dir.join("yt-client.db");
        let db = Db::open(db_path).expect("open db");
        let db = Arc::new(db);
        let api_key = std::env::var("YT_API_KEY").ok();
        let oauth = OAuthConfig::from_env();
        Self { db, http: reqwest::Client::new(), api_key, oauth }
    }

    pub fn effective_api_key(&self) -> Option<String> {
        self.db.get_kv("api_key").ok().flatten().filter(|s| !s.trim().is_empty()).or_else(|| self.api_key.clone())
    }

    pub fn effective_oauth(&self) -> Option<OAuthConfig> {
        let cid = self.db.get_kv("client_id").ok().flatten().filter(|s| !s.trim().is_empty())
            .or_else(|| std::env::var("YT_CLIENT_ID").ok());
        let cid = cid?;
        let csec = self.db.get_kv("client_secret").ok().flatten().filter(|s| !s.trim().is_empty())
            .or_else(|| std::env::var("YT_CLIENT_SECRET").ok());
        Some(OAuthConfig { client_id: cid, client_secret: csec })
    }

    pub fn yt(&self) -> YtClient {
        let token = self.db.get_kv("access_token").ok().flatten();
        YtClient::new(self.effective_api_key()).with_token(token)
    }
}