#![allow(non_snake_case)]
use crate::error::{AppError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const BASE: &str = "https://www.googleapis.com/youtube/v3";

#[derive(Clone)]
pub struct YtClient {
    pub http: Client,
    pub api_key: Option<String>,
    pub access_token: Option<String>,
}

impl YtClient {
    pub fn new(api_key: Option<String>) -> Self {
        Self { http: Client::new(), api_key, access_token: None }
    }
    pub fn with_token(mut self, token: Option<String>) -> Self {
        self.access_token = token;
        self
    }

    fn auth_query(&self) -> Vec<(&str, String)> {
        let mut v = Vec::new();
        if let Some(k) = &self.api_key {
            v.push(("key", k.clone()));
        }
        v
    }

    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str, params: &[(&str, String)]) -> Result<T> {
        let url = format!("{BASE}/{path}");
        let mut req = self.http.get(&url);
        if let Some(t) = &self.access_token {
            req = req.bearer_auth(t);
        }
        let resp = req.query(params).query(&self.auth_query()).send().await?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| AppError::Http(e.to_string()))?;
        if !status.is_success() {
            return Err(AppError::Api(format!("{}: {}", status, text)));
        }
        serde_json::from_str(&text).map_err(|e| AppError::Api(format!("parse: {e}; body: {text}")))
    }

    pub async fn search(&self, q: &str, max: u32, page_token: Option<&str>) -> Result<SearchResponse> {
        let mut params: Vec<(&str, String)> = vec![
            ("part", "snippet".into()),
            ("q", q.into()),
            ("type", "video".into()),
            ("maxResults", max.to_string()),
        ];
        if let Some(pt) = page_token { params.push(("pageToken", pt.into())); }
        self.get("search", &params).await
    }

    pub async fn videos(&self, ids: &[String]) -> Result<VideosResponse> {
        if ids.is_empty() { return Ok(VideosResponse { items: vec![] }); }
        let params = vec![
            ("part", "snippet,contentDetails,statistics".into()),
            ("id", ids.join(",")),
        ];
        self.get("videos", &params).await
    }

    pub async fn channels(&self, ids: &[String]) -> Result<ChannelsResponse> {
        if ids.is_empty() { return Ok(ChannelsResponse { items: vec![] }); }
        let params = vec![
            ("part", "snippet,statistics,contentDetails".into()),
            ("id", ids.join(",")),
        ];
        self.get("channels", &params).await
    }

    pub async fn playlists(&self, channel_id: &str, max: u32) -> Result<PlaylistsResponse> {
        let params = vec![
            ("part", "snippet,contentDetails".into()),
            ("channelId", channel_id.into()),
            ("maxResults", max.to_string()),
        ];
        self.get("playlists", &params).await
    }

    pub async fn playlist_items(&self, playlist_id: &str, max: u32, page_token: Option<&str>) -> Result<PlaylistItemsResponse> {
        let mut params: Vec<(&str, String)> = vec![
            ("part", "snippet,contentDetails".into()),
            ("playlistId", playlist_id.into()),
            ("maxResults", max.to_string()),
        ];
        if let Some(pt) = page_token { params.push(("pageToken", pt.into())); }
        self.get("playlistItems", &params).await
    }

    pub async fn subscriptions(&self, max: u32, page_token: Option<&str>) -> Result<SubscriptionsResponse> {
        let mut params: Vec<(&str, String)> = vec![
            ("part", "snippet,contentDetails".into()),
            ("mine", "true".into()),
            ("maxResults", max.to_string()),
        ];
        if let Some(pt) = page_token { params.push(("pageToken", pt.into())); }
        self.get("subscriptions", &params).await
    }

    pub async fn channel_activities(&self, channel_id: &str, max: u32) -> Result<ActivitiesResponse> {
        let params = vec![
            ("part", "snippet,contentDetails".into()),
            ("channelId", channel_id.into()),
            ("maxResults", max.to_string()),
        ];
        self.get("activities", &params).await
    }

    pub async fn comment_threads(&self, video_id: &str, max: u32, page_token: Option<&str>) -> Result<CommentThreadsResponse> {
        let mut params: Vec<(&str, String)> = vec![
            ("part", "snippet,replies".into()),
            ("videoId", video_id.into()),
            ("maxResults", max.to_string()),
            ("order", "relevance".into()),
            ("textFormat", "plainText".into()),
        ];
        if let Some(pt) = page_token { params.push(("pageToken", pt.into())); }
        self.get("commentThreads", &params).await
    }

    pub async fn related_videos(&self, video_id: &str, max: u32) -> Result<SearchResponse> {
        let params = vec![
            ("part", "snippet".into()),
            ("relatedToVideoId", video_id.into()),
            ("type", "video".into()),
            ("maxResults", max.to_string()),
        ];
        self.get("search", &params).await
    }
}

// ---------- typed responses ----------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PageInfo { pub totalResults: Option<u64>, pub resultsPerPage: Option<u32> }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Thumbnails { pub default: Option<Thumb>, pub medium: Option<Thumb>, pub high: Option<Thumb>, pub standard: Option<Thumb>, pub maxres: Option<Thumb> }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Thumb { pub url: String, pub width: Option<u32>, pub height: Option<u32> }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Snippet {
    pub publishedAt: Option<String>,
    pub channelId: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub thumbnails: Option<Thumbnails>,
    pub channelTitle: Option<String>,
    pub playlistId: Option<String>,
    pub position: Option<u32>,
    pub resourceId: Option<ResourceId>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceId { pub kind: Option<String>, pub videoId: Option<String>, pub channelId: Option<String>, pub playlistId: Option<String> }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RelatedPlaylists {
    pub uploads: Option<String>,
    pub likes: Option<String>,
    pub favorites: Option<String>,
    pub watchHistory: Option<String>,
    pub watchLater: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContentDetails {
    pub duration: Option<String>,
    pub dimension: Option<String>,
    pub definition: Option<String>,
    pub caption: Option<String>,
    pub licensedContent: Option<bool>,
    pub contentRating: Option<serde_json::Value>,
    pub projection: Option<String>,
    pub itemCount: Option<u32>,
    pub videoId: Option<String>,
    pub videoPublishedAt: Option<String>,
    pub relatedPlaylists: Option<RelatedPlaylists>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Statistics {
    pub viewCount: Option<String>,
    pub likeCount: Option<String>,
    pub dislikeCount: Option<String>,
    pub favoriteCount: Option<String>,
    pub commentCount: Option<String>,
    pub subscriberCount: Option<String>,
    pub hiddenSubscriberCount: Option<bool>,
    pub videoCount: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchItem { pub kind: Option<String>, pub etag: Option<String>, pub id: Option<SearchId>, pub snippet: Option<Snippet> }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchId { pub kind: Option<String>, pub videoId: Option<String>, pub channelId: Option<String>, pub playlistId: Option<String> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse { pub kind: Option<String>, pub etag: Option<String>, pub nextPageToken: Option<String>, pub prevPageToken: Option<String>, pub pageInfo: Option<PageInfo>, pub items: Vec<SearchItem> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoItem { pub kind: Option<String>, pub etag: Option<String>, pub id: String, pub snippet: Option<Snippet>, pub contentDetails: Option<ContentDetails>, pub statistics: Option<Statistics> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideosResponse { pub items: Vec<VideoItem> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelItem { pub kind: Option<String>, pub etag: Option<String>, pub id: String, pub snippet: Option<Snippet>, pub statistics: Option<Statistics>, pub contentDetails: Option<ContentDetails> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelsResponse { pub items: Vec<ChannelItem> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistItem { pub kind: Option<String>, pub etag: Option<String>, pub id: String, pub snippet: Option<Snippet>, pub contentDetails: Option<ContentDetails> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistsResponse { pub nextPageToken: Option<String>, pub pageInfo: Option<PageInfo>, pub items: Vec<PlaylistItem> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistEntry { pub kind: Option<String>, pub etag: Option<String>, pub id: String, pub snippet: Option<Snippet>, pub contentDetails: Option<ContentDetails> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistItemsResponse { pub nextPageToken: Option<String>, pub pageInfo: Option<PageInfo>, pub items: Vec<PlaylistEntry> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionItem { pub kind: Option<String>, pub etag: Option<String>, pub id: String, pub snippet: Option<Snippet>, pub contentDetails: Option<ContentDetails> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionsResponse { pub nextPageToken: Option<String>, pub pageInfo: Option<PageInfo>, pub items: Vec<SubscriptionItem> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityItem { pub kind: Option<String>, pub etag: Option<String>, pub id: String, pub snippet: Option<Snippet>, pub contentDetails: Option<serde_json::Value> }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivitiesResponse { pub nextPageToken: Option<String>, pub pageInfo: Option<PageInfo>, pub items: Vec<ActivityItem> }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommentSnippet {
    pub authorDisplayName: Option<String>,
    pub authorProfileImageUrl: Option<String>,
    pub authorChannelUrl: Option<String>,
    pub textDisplay: Option<String>,
    pub textOriginal: Option<String>,
    pub likeCount: Option<u32>,
    pub publishedAt: Option<String>,
    pub updatedAt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Comment {
    pub id: Option<String>,
    pub snippet: Option<CommentSnippet>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommentThreadSnippet {
    pub channelId: Option<String>,
    pub videoId: Option<String>,
    pub topLevelComment: Option<Comment>,
    pub canReply: Option<bool>,
    pub totalReplyCount: Option<u32>,
    pub isPublic: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentThread {
    pub kind: Option<String>,
    pub etag: Option<String>,
    pub id: String,
    pub snippet: Option<CommentThreadSnippet>,
    pub replies: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentThreadsResponse {
    pub nextPageToken: Option<String>,
    pub pageInfo: Option<PageInfo>,
    pub items: Vec<CommentThread>,
}

// helper to extract best thumbnail url
pub fn best_thumb(t: &Option<Thumbnails>) -> Option<String> {
    let t = t.as_ref()?;
    t.maxres.as_ref().or(t.standard.as_ref()).or(t.high.as_ref()).or(t.medium.as_ref()).or(t.default.as_ref()).map(|x| x.url.clone())
}

// conversion helpers used by commands
pub fn _arc<T>(x: T) -> Arc<T> { Arc::new(x) }