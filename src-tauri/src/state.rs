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

    pub fn yt(&self) -> YtClient {
        let token = self.db.get_kv("access_token").ok().flatten();
        YtClient::new(self.api_key.clone()).with_token(token)
    }
}