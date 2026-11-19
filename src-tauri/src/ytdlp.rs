use crate::error::{AppError, Result};
use crate::commands::VideoView;
use serde_json::Value;
use std::process::Stdio;

fn ytdlp_path() -> String {
    // prefer system yt-dlp
    "yt-dlp".to_string()
}

pub async fn run_raw(args: &[&str]) -> Result<String> {
    run(args).await
}

async fn run(args: &[&str]) -> Result<String> {
    let mut cmd = tokio::process::Command::new(ytdlp_path());
    cmd.args(args)
        .arg("--js-runtimes").arg("node")
        .arg("--no-warnings")
        .env_remove("PYTHONHOME")
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONSTARTUP")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let out = cmd.output().await.map_err(|e| AppError::Io(format!("yt-dlp: {e}")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        return Err(AppError::Api(format!("yt-dlp failed: {err}")));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn thumb(d: &Value) -> Option<String> {
    if let Some(t) = d.get("thumbnail").and_then(|x| x.as_str()) { return Some(t.to_string()); }
    d.get("thumbnails").and_then(|x| x.as_array()).and_then(|a| a.last()).and_then(|t| t.get("url")).and_then(|x| x.as_str()).map(|s| s.to_string())
}

fn dur_str(secs: Option<u64>) -> Option<String> {
    secs.map(|s| {
        let h = s / 3600;
        let m = (s % 3600) / 60;
        let sec = s % 60;
        if h > 0 { format!("PT{}H{}M{}S", h, m, sec) } else { format!("PT{}M{}S", m, sec) }
    })
}

pub fn to_video_view(d: &Value) -> VideoView {
    let id = d.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
    VideoView {
        id,
        title: d.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        channel_id: d.get("channel_id").or_else(|| d.get("uploader_id")).and_then(|x| x.as_str()).map(|s| s.to_string()),
        channel_title: d.get("channel").or_else(|| d.get("uploader")).and_then(|x| x.as_str()).map(|s| s.to_string()),
        description: d.get("description").and_then(|x| x.as_str()).map(|s| s.to_string()),
        thumbnail: thumb(d),
        duration: dur_str(d.get("duration").and_then(|x| x.as_f64()).map(|f| f as u64)),
        published_at: d.get("upload_date").and_then(|x| x.as_str()).map(|s| format!("{}-{}-{}", &s[0..4], &s[4..6], &s[6..8])),
        view_count: d.get("view_count").map(|x| x.to_string()),
    }
}

pub async fn search(query: &str, max: usize) -> Result<Vec<VideoView>> {
    let spec = format!("ytsearch{}:{}", max, query);
    // --flat-playlist is much faster: only the search results page, no per-video fetches.
    let out = run(&["--dump-json", "--flat-playlist", &spec]).await?;
    let mut items = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            items.push(to_video_view(&v));
        }
    }
    Ok(items)
}

pub async fn get_video(id: &str) -> Result<VideoView> {
    let url = format!("https://www.youtube.com/watch?v={}", id);
    let out = run(&["--dump-json", "--skip-download", &url]).await?;
    let v: Value = serde_json::from_str(out.trim()).map_err(|e| AppError::Api(e.to_string()))?;
    Ok(to_video_view(&v))
}

pub async fn get_comments(id: &str, max: usize) -> Result<Vec<serde_json::Value>> {
    let url = format!("https://www.youtube.com/watch?v={}", id);
    let arg = format!("youtube:max_comments={},all", max);
    let out = run(&["--dump-json", "--get-comments", "--skip-download", "--extractor-args", &arg, &url]).await?;
    let v: Value = serde_json::from_str(out.trim()).map_err(|e| AppError::Api(e.to_string()))?;
    let mut items = Vec::new();
    if let Some(arr) = v.get("comments").and_then(|x| x.as_array()) {
        for c in arr {
            items.push(serde_json::json!({
                "id": c.get("id"),
                "author": c.get("author"),
                "avatar": c.get("author_thumbnail"),
                "text": c.get("text"),
                "likes": c.get("like_count"),
                "publishedAt": c.get("timestamp").map(|t| t.to_string()),
            }));
        }
    }
    Ok(items)
}

pub async fn get_related(id: &str, title: &str) -> Result<Vec<VideoView>> {
    // poor-man's related: search by title + channel-less query
    let q = title.to_string();
    let items = search(&q, 8).await.unwrap_or_default();
    Ok(items.into_iter().filter(|v| v.id != id).collect())
}

pub async fn get_channel_videos(channel_id: &str) -> Result<Vec<VideoView>> {
    // channel uploads via yt-dlp on channel URL
    let url = if channel_id.starts_with("http") {
        channel_id.to_string()
    } else {
        format!("https://www.youtube.com/channel/{}", channel_id)
    };
    let out = run(&["--dump-json", "--flat-playlist", &url]).await;
    let mut items = Vec::new();
    if let Ok(o) = out {
        for line in o.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            if let Ok(v) = serde_json::from_str::<Value>(line) {
                items.push(to_video_view(&v));
            }
        }
    }
    // flat-playlist may not include full meta; that's ok
    Ok(items)
}