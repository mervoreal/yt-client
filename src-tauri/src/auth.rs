use crate::db::Db;
use crate::error::{AppError, Result};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::Rng;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const SCOPES: &str = "https://www.googleapis.com/auth/youtube.readonly https://www.googleapis.com/auth/youtube";

#[derive(Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: Option<String>,
}

impl OAuthConfig {
    pub fn from_env() -> Option<Self> {
        let client_id = std::env::var("YT_CLIENT_ID").ok()?;
        let client_secret = std::env::var("YT_CLIENT_SECRET").ok();
        Some(Self { client_id, client_secret })
    }
}

fn gen_verifier() -> String {
    let bytes: Vec<u8> = (0..32).map(|_| rand::random::<u8>()).collect();
    URL_SAFE_NO_PAD.encode(bytes)
}

fn challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hash)
}

pub async fn run_oauth_flow(cfg: OAuthConfig, db: Arc<Db>, opener: impl Fn(&str)) -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| AppError::Io(e.to_string()))?;
    let port = listener.local_addr().map_err(|e| AppError::Io(e.to_string()))?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    let verifier = gen_verifier();
    let chall = challenge(&verifier);
    let state: String = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 16]>());

    let auth_url = format!(
        "{AUTH_URL}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256&state={}&access_type=offline&prompt=consent",
        urlencoding(&cfg.client_id),
        urlencoding(&redirect_uri),
        urlencoding(SCOPES),
        urlencoding(&chall),
        urlencoding(&state),
    );
    opener(&auth_url);

    // wait for callback
    let (mut socket, _) = listener.accept().await.map_err(|e| AppError::Io(e.to_string()))?;
    let mut buf = vec![0u8; 4096];
    let n = socket.read(&mut buf).await.map_err(|e| AppError::Io(e.to_string()))?;
    let req = String::from_utf8_lossy(&buf[..n]).to_string();

    // parse request line
    let first_line = req.lines().next().unwrap_or("");
    // GET /callback?code=...&state=... HTTP/1.1
    let path = first_line.split_whitespace().nth(1).unwrap_or("");
    let query = path.split('?').nth(1).unwrap_or("");
    let mut code: Option<String> = None;
    let mut got_state: Option<String> = None;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        let k = kv.next().unwrap_or("");
        let v = kv.next().unwrap_or("");
        let v = urldecoding(v);
        match k {
            "code" => code = Some(v),
            "state" => got_state = Some(v),
            "error" => return Err(AppError::Auth(format!("oauth error: {v}"))),
            _ => {}
        }
    }
    if got_state.as_deref() != Some(&state) {
        return Err(AppError::Auth("state mismatch".into()));
    }
    let code = code.ok_or_else(|| AppError::Auth("no code in callback".into()))?;

    // respond to browser
    let body = "<html><body>Authorized. You can close this window.</body></html>";
    let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
    let _ = socket.write_all(resp.as_bytes()).await;

    // exchange code for tokens
    let mut form: Vec<(&str, String)> = vec![
        ("grant_type", "authorization_code".into()),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", cfg.client_id.clone()),
        ("code_verifier", verifier),
    ];
    if let Some(s) = &cfg.client_secret {
        form.push(("client_secret", s.clone()));
    }
    let res = reqwest::Client::new()
        .post(TOKEN_URL)
        .form(&form)
        .send().await
        .map_err(|e| AppError::Http(e.to_string()))?;
    let status = res.status();
    let text = res.text().await.map_err(|e| AppError::Http(e.to_string()))?;
    if !status.is_success() {
        return Err(AppError::Auth(format!("token exchange failed: {status} {text}")));
    }
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| AppError::Auth(e.to_string()))?;
    let access = json.get("access_token").and_then(|x| x.as_str()).ok_or_else(|| AppError::Auth("no access_token".into()))?.to_string();
    if let Some(r) = json.get("refresh_token").and_then(|x| x.as_str()) {
        db.set_kv("refresh_token", r)?;
    }
    if let Some(e) = json.get("expires_in").and_then(|x| x.as_i64()) {
        let exp = chrono::Utc::now().timestamp() + e;
        db.set_kv("expires_at", &exp.to_string())?;
    }
    db.set_kv("access_token", &access)?;
    Ok(access)
}

pub async fn refresh_if_needed(cfg: OAuthConfig, db: Arc<Db>) -> Result<Option<String>> {
    // check expiry
    let exp = db.get_kv("expires_at")?.and_then(|s| s.parse::<i64>().ok());
    let now = chrono::Utc::now().timestamp();
    let access = db.get_kv("access_token")?;
    if let (Some(e), Some(a)) = (exp, access.clone()) {
        if now < e - 60 { return Ok(Some(a)); }
    }
    // try refresh
    let refresh = match db.get_kv("refresh_token")? { Some(r) => r, None => return Ok(access) };
    let mut form: Vec<(&str, String)> = vec![
        ("grant_type", "refresh_token".into()),
        ("refresh_token", refresh),
        ("client_id", cfg.client_id.clone()),
    ];
    if let Some(s) = &cfg.client_secret { form.push(("client_secret", s.clone())); }
    let res = reqwest::Client::new().post(TOKEN_URL).form(&form).send().await.map_err(|e| AppError::Http(e.to_string()))?;
    let status = res.status();
    let text = res.text().await.map_err(|e| AppError::Http(e.to_string()))?;
    if !status.is_success() { return Ok(access); }
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| AppError::Auth(e.to_string()))?;
    if let Some(a) = json.get("access_token").and_then(|x| x.as_str()) {
        db.set_kv("access_token", a)?;
        if let Some(e) = json.get("expires_in").and_then(|x| x.as_i64()) {
            db.set_kv("expires_at", &(chrono::Utc::now().timestamp() + e).to_string())?;
        }
        return Ok(Some(a.to_string()));
    }
    Ok(access)
}

pub fn current_token(db: &Db) -> Option<String> {
    db.get_kv("access_token").ok().flatten()
}

fn urlencoding(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
fn urldecoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() + 1 && i + 2 < bytes.len() {
            // need i+2 index valid: i+2 < len
        }
        if bytes[i] == b'%' && i + 3 <= bytes.len() {
            let hex = &s[i + 1..i + 3];
            if let Ok(b) = u8::from_str_radix(hex, 16) {
                out.push(b as char);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(' ');
        } else {
            out.push(bytes[i] as char);
        }
        i += 1;
    }
    out
}