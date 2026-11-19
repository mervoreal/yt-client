use crate::auth;
use crate::error::Result;
use crate::state::AppState;
use crate::youtube::*;
use serde::Serialize;
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
pub async fn search_videos(state: State<'_, AppState>, query: String, page_token: Option<String>) -> Result<serde_json::Value> {
    let yt = state.yt();
    let resp = yt.search(&query, 20, page_token.as_deref()).await?;
    // fetch full video details for durations
    let ids: Vec<String> = resp.items.iter().filter_map(|i| i.id.as_ref()?.videoId.clone()).collect();
    let details = yt.videos(&ids).await.unwrap_or(VideosResponse { items: vec![] });
    let map: std::collections::HashMap<String, VideoItem> = details.items.into_iter().map(|v| (v.id.clone(), v)).collect();
    let items: Vec<VideoView> = ids.iter().filter_map(|id| map.get(id).cloned().map(VideoView::from).or_else(|| {
        // fallback from search snippet
        resp.items.iter().find(|s| s.id.as_ref().and_then(|x| x.videoId.as_ref()) == Some(id)).map(|s| {
            let sn = s.snippet.clone().unwrap_or_default();
            VideoView {
                id: id.clone(),
                title: sn.title.unwrap_or_default(),
                channel_id: sn.channelId,
                channel_title: sn.channelTitle,
                description: sn.description,
                thumbnail: best_thumb(&sn.thumbnails),
                duration: None,
                published_at: sn.publishedAt,
                view_count: None,
            }
        })
    })).collect();
    let next = resp.nextPageToken;
    Ok(serde_json::json!({ "items": items, "nextPageToken": next }))
}

#[tauri::command]
pub async fn get_video(state: State<'_, AppState>, id: String) -> Result<VideoView> {
    let yt = state.yt();
    let resp = yt.videos(&[id.clone()]).await?;
    let v = resp.items.into_iter().next().ok_or_else(|| crate::error::AppError::NotFound(id))?;
    Ok(VideoView::from(v))
}

#[tauri::command]
pub async fn get_channel(state: State<'_, AppState>, id: String) -> Result<serde_json::Value> {
    let yt = state.yt();
    let resp = yt.channels(&[id.clone()]).await?;
    let c = resp.items.into_iter().next().ok_or_else(|| crate::error::AppError::NotFound(id))?;
    let s = c.snippet.unwrap_or_default();
    Ok(serde_json::json!({
        "id": c.id,
        "title": s.title,
        "description": s.description,
        "thumbnail": best_thumb(&s.thumbnails),
        "subscriberCount": c.statistics.as_ref().and_then(|x| x.subscriberCount.clone()),
        "videoCount": c.statistics.as_ref().and_then(|x| x.videoCount.clone()),
        "uploadsPlaylistId": c.contentDetails.as_ref().and_then(|cd| cd.relatedPlaylists.as_ref()).and_then(|r| r.uploads.clone()),
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
pub async fn get_playlist_items(state: State<'_, AppState>, playlist_id: String, page_token: Option<String>) -> Result<serde_json::Value> {
    let yt = state.yt();
    let resp = yt.playlist_items(&playlist_id, 25, page_token.as_deref()).await?;
    let ids: Vec<String> = resp.items.iter().filter_map(|e| e.contentDetails.as_ref()?.videoId.clone().or_else(|| e.snippet.as_ref()?.resourceId.as_ref()?.videoId.clone())).collect();
    let details = yt.videos(&ids).await.unwrap_or(VideosResponse { items: vec![] });
    let map: std::collections::HashMap<String, VideoItem> = details.items.into_iter().map(|v| (v.id.clone(), v)).collect();
    let items: Vec<VideoView> = ids.iter().filter_map(|id| map.get(id).cloned().map(VideoView::from)).collect();
    Ok(serde_json::json!({ "items": items, "nextPageToken": resp.nextPageToken }))
}

#[tauri::command]
pub async fn get_subscriptions(state: State<'_, AppState>, page_token: Option<String>) -> Result<serde_json::Value> {
    let yt = state.yt();
    let resp = yt.subscriptions(25, page_token.as_deref()).await?;
    let items: Vec<serde_json::Value> = resp.items.into_iter().map(|s| {
        let sn = s.snippet.unwrap_or_default();
        let rid = sn.resourceId.clone().unwrap_or_default();
        serde_json::json!({
            "id": s.id,
            "title": sn.title,
            "thumbnail": best_thumb(&sn.thumbnails),
            "channelId": rid.channelId,
        })
    }).collect();
    Ok(serde_json::json!({ "items": items, "nextPageToken": resp.nextPageToken }))
}

#[tauri::command]
pub async fn get_subscription_feed(state: State<'_, AppState>, channel_id: String) -> Result<serde_json::Value> {
    // use the channel's uploads playlist
    let yt = state.yt();
    let ch = yt.channels(&[channel_id.clone()]).await?;
    let c = ch.items.into_iter().next().ok_or_else(|| crate::error::AppError::NotFound(channel_id.clone()))?;
    let uploads = c.contentDetails.as_ref().and_then(|cd| cd.relatedPlaylists.as_ref()).and_then(|r| r.uploads.clone()).ok_or_else(|| crate::error::AppError::NotFound("uploads".into()))?;
    get_playlist_items(state, uploads, None).await
}

#[tauri::command]
pub async fn auth_status(state: State<'_, AppState>) -> Result<serde_json::Value> {
    let token = state.db.get_kv("access_token").ok().flatten();
    Ok(serde_json::json!({ "authenticated": token.is_some() }))
}

#[tauri::command]
pub async fn auth_login(state: State<'_, AppState>, app: tauri::AppHandle) -> Result<String> {
    let cfg = state.oauth.clone().ok_or_else(|| crate::error::AppError::Auth("YT_CLIENT_ID not set".into()))?;
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