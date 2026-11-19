mod auth;
mod commands;
mod db;
mod error;
mod state;
mod youtube;
mod ytdlp;

use state::AppState;
use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();
    let state = AppState::new();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::search_videos,
            commands::get_video,
            commands::get_channel,
            commands::get_channel_playlists,
            commands::get_playlist_items,
            commands::get_subscriptions,
            commands::get_subscription_feed,
            commands::auth_status,
            commands::auth_login,
            commands::auth_logout,
            commands::get_settings,
            commands::set_settings,
            commands::check_mpv,
            commands::play_in_mpv,
            commands::get_comments,
            commands::get_related,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}