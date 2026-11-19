use crate::auth;
use crate::error::Result;
use crate::state::AppState;
use crate::youtube::*;
use serde::Serialize;
use std::process::Stdio;
use tauri::State;

#[derive(Serialize)]
pub struct VideoView {
    pub id: String,
    pub title: String,
    pub channel_id: Option<String>,
    pub channel_title: Option<String>,
    pub description: Option<String>,
    pub thumbnail: Option<String>,
    pub duration: Option<String>,
    pub published_at: Option<String>,
    pub view_count: Option<String>,
}

impl From<VideoItem> for VideoView {
    fn from(v: VideoItem) -> Self {
        let s = v.snippet.unwrap_or_default();
        let cd = v.contentDetails.unwrap_or_default();
        let st = v.statistics.unwrap_or_default();
        Self {
            id: v.id,
            title: s.title.unwrap_or_default(),
            channel_id: s.channelId,
            channel_title: s.channelTitle,
            description: s.description,
            thumbnail: best_thumb(&s.thumbnails),
            duration: cd.duration,
            published_at: s.publishedAt,
            view_count: st.viewCount,
        }
    }
}

#[tauri::command]
pub async fn search_videos(_state: State<'_, AppState>, query: String, _page_token: Option<String>) -> Result<serde_json::Value> {
    let items = crate::ytdlp::search(&query, 12).await?;
    Ok(serde_json::json!({ "items": items, "nextPageToken": null }))
}

#[tauri::command]
pub async fn get_video(_state: State<'_, AppState>, id: String) -> Result<VideoView> {
    crate::ytdlp::get_video(&id).await
}

#[tauri::command]
pub async fn get_channel(_state: State<'_, AppState>, id: String) -> Result<serde_json::Value> {
    let url = format!("https://www.youtube.com/channel/{}", id);
    // fetch first video for channel name/thumb
    let first = crate::ytdlp::run_raw(&["--dump-json", "--playlist-end", "1", &url]).await.ok();
    let (title, thumbnail) = if let Some(o) = first {
        let line = o.lines().next().unwrap_or("");
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            (v.get("channel").or_else(|| v.get("uploader")).and_then(|x| x.as_str()).map(|s| s.to_string()).unwrap_or_else(|| id.clone()),
             v.get("thumbnail").and_then(|x| x.as_str()).map(|s| s.to_string()))
        } else { (id.clone(), None) }
    } else { (id.clone(), None) };
    let videos = crate::ytdlp::get_channel_videos(&id).await.unwrap_or_default();
    Ok(serde_json::json!({
        "id": id,
        "title": title,
        "description": null,
        "thumbnail": thumbnail,
        "subscriberCount": null,
        "videoCount": null,
        "uploadsPlaylistId": null,
        "videos": videos,
    }))
}

#[tauri::command]
pub async fn get_channel_playlists(state: State<'_, AppState>, channel_id: String) -> Result<serde_json::Value> {
    let yt = state.yt();
    let resp = yt.playlists(&channel_id, 25).await?;
    let items: Vec<serde_json::Value> = resp.items.into_iter().map(|p| {
        let s = p.snippet.unwrap_or_default();
        serde_json::json!({
            "id": p.id,
            "title": s.title,
            "thumbnail": best_thumb(&s.thumbnails),
            "itemCount": p.contentDetails.as_ref().and_then(|cd| cd.itemCount),
        })
    }).collect();
    Ok(serde_json::json!({ "items": items }))
}

#[tauri::command]
pub async fn get_playlist_items(_state: State<'_, AppState>, playlist_id: String, _page_token: Option<String>) -> Result<serde_json::Value> {
    // Treat playlist_id as a channel id for yt-dlp fallback.
    let items = crate::ytdlp::get_channel_videos(&playlist_id).await.unwrap_or_default();
    Ok(serde_json::json!({ "items": items, "nextPageToken": null }))
}

#[tauri::command]
pub async fn get_subscriptions(_state: State<'_, AppState>, _page_token: Option<String>) -> Result<serde_json::Value> {
    // Try yt-dlp with browser cookies (no API key needed).
    for browser in ["chrome", "firefox", "brave", "edge"] {
        let cookie_arg = format!("--cookies-from-browser={}", browser);
        let out = tokio::process::Command::new("yt-dlp")
            .args(["--dump-json", "--flat-playlist", &cookie_arg, "--playlist-end", "50", "https://www.youtube.com/feed/subscriptions"])
            .arg("--js-runtimes").arg("node").arg("--no-warnings")
            .output().await;
        if let Ok(o) = out {
            if o.status.success() {
                let mut items = Vec::new();
                for line in String::from_utf8_lossy(&o.stdout).lines() {
                    let line = line.trim();
                    if line.is_empty() { continue; }
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                        items.push(serde_json::json!({
                            "id": v.get("id"),
                            "title": v.get("title"),
                            "thumbnail": v.get("thumbnail").or_else(|| v.get("thumbnails").and_then(|t| t.as_array()).and_then(|a| a.last()).and_then(|x| x.get("url"))),
                            "channelId": v.get("channel_id").or_else(|| v.get("uploader_id")),
                        }));
                    }
                }
                if !items.is_empty() {
                    return Ok(serde_json::json!({ "items": items, "nextPageToken": null }));
                }
            }
        }
    }
    Ok(serde_json::json!({ "items": [], "nextPageToken": null }))
}

#[tauri::command]
pub async fn get_subscription_feed(_state: State<'_, AppState>, channel_id: String) -> Result<serde_json::Value> {
    let items = crate::ytdlp::get_channel_videos(&channel_id).await.unwrap_or_default();
    Ok(serde_json::json!({ "items": items, "nextPageToken": null }))
}

#[tauri::command]
pub async fn auth_status(state: State<'_, AppState>) -> Result<serde_json::Value> {
    let token = state.db.get_kv("access_token").ok().flatten();
    Ok(serde_json::json!({ "authenticated": token.is_some() && !token.unwrap_or_default().is_empty() }))
}

#[tauri::command]
pub async fn auth_login(state: State<'_, AppState>, app: tauri::AppHandle) -> Result<String> {
    let cfg = state.effective_oauth().ok_or_else(|| crate::error::AppError::Auth("Client ID not set. Open Settings.".into()))?;
    let db = state.db.clone();
    let opener = |url: &str| { let _ = tauri_plugin_opener::open_url(url, None::<&str>); };
    let token = auth::run_oauth_flow(cfg, db, opener).await?;
    Ok(token)
}

#[tauri::command]
pub async fn auth_logout(state: State<'_, AppState>) -> Result<()> {
    state.db.set_kv("access_token", "")?;
    state.db.set_kv("refresh_token", "")?;
    Ok(())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<serde_json::Value> {
    Ok(serde_json::json!({
        "api_key": state.db.get_kv("api_key").ok().flatten().unwrap_or_default(),
        "client_id": state.db.get_kv("client_id").ok().flatten().unwrap_or_default(),
        "client_secret": state.db.get_kv("client_secret").ok().flatten().unwrap_or_default(),
        "has_api_key": state.effective_api_key().is_some(),
        "has_client_id": state.effective_oauth().is_some(),
    }))
}

#[tauri::command]
pub async fn set_settings(state: State<'_, AppState>, api_key: Option<String>, client_id: Option<String>, client_secret: Option<String>) -> Result<()> {
    if let Some(v) = api_key { state.db.set_kv("api_key", &v)?; }
    if let Some(v) = client_id { state.db.set_kv("client_id", &v)?; }
    if let Some(v) = client_secret { state.db.set_kv("client_secret", &v)?; }
    Ok(())
}

#[tauri::command]
pub async fn get_comments(_state: State<'_, AppState>, video_id: String, _page_token: Option<String>) -> Result<serde_json::Value> {
    let items = crate::ytdlp::get_comments(&video_id, 20).await?;
    Ok(serde_json::json!({ "items": items, "nextPageToken": null }))
}

#[tauri::command]
pub async fn get_related(_state: State<'_, AppState>, video_id: String) -> Result<serde_json::Value> {
    // need title for related search; fetch video first
    let v = crate::ytdlp::get_video(&video_id).await.ok();
    let title = v.map(|x| x.title).unwrap_or_default();
    let items = crate::ytdlp::get_related(&video_id, &title).await.unwrap_or_default();
    Ok(serde_json::json!({ "items": items }))
}

#[tauri::command]
pub async fn record_watch(state: State<'_, AppState>, video_id: String, title: String, channel_title: Option<String>, thumbnail: Option<String>, progress: Option<f64>) -> Result<()> {
    state.db.record_history(&video_id, &title, channel_title.as_deref(), thumbnail.as_deref(), progress.unwrap_or(0.0))?;
    Ok(())
}

#[tauri::command]
pub async fn get_history(state: State<'_, AppState>) -> Result<serde_json::Value> {
    let rows = state.db.get_history(50)?;
    let items: Vec<serde_json::Value> = rows.into_iter().map(|(id,title,channel,thumb,watched,progress)| {
        serde_json::json!({
            "id": id, "title": title, "channel_title": channel, "thumbnail": thumb,
            "watched_at": watched, "progress": progress,
        })
    }).collect();
    Ok(serde_json::json!({ "items": items }))
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<()> {
    state.db.clear_history()?;
    Ok(())
}

#[tauri::command]
pub async fn get_thumbnail(state: State<'_, AppState>, url: String) -> Result<String> {
    // check cache
    if let Ok(Some(path)) = state.db.get_kv(&format!("thumb:{}", url)) {
        if let Ok(bytes) = std::fs::read(&path) {
            let mime = if path.ends_with(".webp") { "image/webp" } else if path.ends_with(".png") { "image/png" } else { "image/jpeg" };
            use base64::Engine;
            return Ok(format!("data:{};base64,{}", mime, base64::engine::general_purpose::STANDARD.encode(bytes)));
        }
    }
    // download
    let resp = reqwest::get(&url).await.map_err(|e| crate::error::AppError::Http(e.to_string()))?;
    let bytes = resp.bytes().await.map_err(|e| crate::error::AppError::Http(e.to_string()))?;
    let cache_dir = dirs::cache_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join("yt-client").join("thumbs");
    std::fs::create_dir_all(&cache_dir).map_err(|e| crate::error::AppError::Io(e.to_string()))?;
    // filename from url hash
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    url.hash(&mut h);
    let ext = if url.contains(".webp") { "webp" } else if url.contains(".png") { "png" } else { "jpg" };
    let path = cache_dir.join(format!("{:x}.{}", h.finish(), ext));
    std::fs::write(&path, &bytes).map_err(|e| crate::error::AppError::Io(e.to_string()))?;
    let _ = state.db.set_kv(&format!("thumb:{}", url), &path.to_string_lossy());
    let mime = if ext == "webp" { "image/webp" } else if ext == "png" { "image/png" } else { "image/jpeg" };
    use base64::Engine;
    Ok(format!("data:{};base64,{}", mime, base64::engine::general_purpose::STANDARD.encode(bytes)))
}

#[tauri::command]
pub async fn download_video(_state: State<'_, AppState>, video_id: String) -> Result<String> {
    let url = format!("https://www.youtube.com/watch?v={}", video_id);
    let dir = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join("yt-client").join("downloads");
    std::fs::create_dir_all(&dir).map_err(|e| crate::error::AppError::Io(e.to_string()))?;
    let out_template = dir.join("%(id)s.%(ext)s");
    let status = tokio::process::Command::new("yt-dlp")
        .args(["-f", "best[acodec!=none][vcodec!=none]/best", "-o", out_template.to_str().unwrap(), "--js-runtimes", "node", "--no-warnings", &url])
        .status().await.map_err(|e| crate::error::AppError::Io(e.to_string()))?;
    if !status.success() { return Err(crate::error::AppError::Api("download failed".into())); }
    // find the file
    for entry in std::fs::read_dir(&dir).map_err(|e| crate::error::AppError::Io(e.to_string()))? {
        let e = entry.map_err(|e| crate::error::AppError::Io(e.to_string()))?;
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with(&video_id) { return Ok(e.path().to_string_lossy().to_string()); }
    }
    Err(crate::error::AppError::NotFound("downloaded file".into()))
}

#[tauri::command]
pub async fn play_local(path: String) -> Result<()> {
    let mut cmd = std::process::Command::new("mpv");
    cmd.arg(&path).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    cmd.spawn().map_err(|e| crate::error::AppError::Io(format!("mpv: {e}")))?;
    Ok(())
}

#[tauri::command]
pub async fn pip_play(video_id: String) -> Result<()> {
    let url = format!("https://www.youtube.com/watch?v={}", video_id);
    let mut cmd = std::process::Command::new("mpv");
    cmd.args(["--ontop", "--no-border", "--geometry=320x180+100+100", "--autofit=320x180", &url])
        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    cmd.spawn().map_err(|e| crate::error::AppError::Io(format!("mpv: {e}")))?;
    Ok(())
}

#[tauri::command]
pub async fn search_suggestions(_state: State<'_, AppState>, query: String) -> Result<serde_json::Value> {
    if query.trim().is_empty() { return Ok(serde_json::json!({ "items": [] })); }
    let url = format!("https://suggestqueries.google.com/complete/search?client=youtube&ds=yt&q={}", urlencoding(&query));
    let text = reqwest::get(&url).await.map_err(|e| crate::error::AppError::Http(e.to_string()))?.text().await.map_err(|e| crate::error::AppError::Http(e.to_string()))?;
    // parse JSONP: ...([...])
    let mut items = Vec::new();
    // find array of suggestions: "suggestions":[["...",0],...]
    if let Some(start) = text.find("suggestions") {
        let rest = &text[start..];
        // find first '[' after suggestions
        if let Some(bracket) = rest.find('[') {
            let arr_str = &rest[bracket..];
            // find matching close by scanning
            let mut depth = 0;
            let mut end = 0;
            for (i, ch) in arr_str.char_indices() {
                if ch == '[' { depth += 1; }
                else if ch == ']' { depth -= 1; if depth == 0 { end = i + 1; break; } }
            }
            if end > 0 {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&arr_str[..end]) {
                    if let Some(arr) = val.as_array() {
                        for entry in arr {
                            if let Some(s) = entry.get(0).and_then(|x| x.as_str()) {
                                items.push(s.to_string());
                            } else if let Some(s) = entry.as_str() {
                                items.push(s.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(serde_json::json!({ "items": items }))
}

fn urlencoding(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

#[tauri::command]
pub async fn like_video(state: State<'_, AppState>, video_id: String, like: bool) -> Result<()> {
    let yt = state.yt();
    let token = state.db.get_kv("access_token").ok().flatten().filter(|s| !s.is_empty()).ok_or_else(|| crate::error::AppError::Auth("not logged in".into()))?;
    let _ = token;
    // Use YouTube Data API videos.rate
    let rating = if like { "like" } else { "none" };
    let url = format!("https://www.googleapis.com/youtube/v3/videos/rate?id={}&rating={}", video_id, rating);
    let client = reqwest::Client::new();
    let resp = client.post(&url).bearer_auth(state.db.get_kv("access_token").ok().flatten().unwrap_or_default()).send().await.map_err(|e| crate::error::AppError::Http(e.to_string()))?;
    if !resp.status().is_success() {
        let t = resp.text().await.unwrap_or_default();
        return Err(crate::error::AppError::Api(format!("rate failed: {t}")));
    }
    let _ = yt;
    Ok(())
}

#[tauri::command]
pub async fn add_to_watch_later(state: State<'_, AppState>, video_id: String) -> Result<()> {
    // watch later playlist id is "WL"
    let body = serde_json::json!({
        "snippet": {
            "playlistId": "WL",
            "resourceId": { "kind": "youtube#video", "videoId": video_id }
        }
    });
    let client = reqwest::Client::new();
    let resp = client.post("https://www.googleapis.com/youtube/v3/playlistItems?part=snippet")
        .bearer_auth(state.db.get_kv("access_token").ok().flatten().unwrap_or_default())
        .json(&body)
        .send().await.map_err(|e| crate::error::AppError::Http(e.to_string()))?;
    if !resp.status().is_success() {
        let t = resp.text().await.unwrap_or_default();
        return Err(crate::error::AppError::Api(format!("watch later failed: {t}")));
    }
    Ok(())
}

#[tauri::command]
pub async fn mini_player(video_id: String) -> Result<()> {
    let url = format!("https://www.youtube.com/watch?v={}", video_id);
    let mut cmd = std::process::Command::new("mpv");
    cmd.args(["--geometry=480x270", "--ontop", "--title=Mini Player", &url])
        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    cmd.spawn().map_err(|e| crate::error::AppError::Io(format!("mpv: {e}")))?;
    Ok(())
}

#[tauri::command]
pub async fn check_mpv() -> Result<bool> {
    // check if mpv is on PATH
    let status = std::process::Command::new("mpv").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status();
    Ok(status.map(|s| s.success()).unwrap_or(false))
}

#[tauri::command]
pub async fn play_in_mpv(video_id: String) -> Result<()> {
    let url = format!("https://www.youtube.com/watch?v={}", video_id);
    // spawn mpv detached so it doesn't block
    let mut cmd = std::process::Command::new("mpv");
    cmd.arg(&url)
        .arg("--force-window=immediate")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // detach
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn().map_err(|e| crate::error::AppError::Io(format!("mpv not found or failed to start: {e}")))?;
    Ok(())
}