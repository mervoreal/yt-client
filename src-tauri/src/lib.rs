mod auth;
mod commands;
mod db;
mod error;
mod state;
mod youtube;
mod ytdlp;

use state::AppState;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();
    let state = AppState::new();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(state)
        .setup(|app| {
            let handle = app.handle().clone();
            // Media keys -> emit event to frontend
            let shortcuts = [
                Shortcut::new(None, Code::MediaPlayPause),
                Shortcut::new(None, Code::MediaTrackNext),
                Shortcut::new(None, Code::MediaTrackPrevious),
            ];
            let gs = app.global_shortcut();
            for s in shortcuts {
                let h = handle.clone();
                let _ = gs.on_shortcut(s, move |_app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        let _ = h.emit("media-key", format!("{:?}", _shortcut.key));
                    }
                });
            }
            let _ = gs.register(Shortcut::new(None, Code::MediaPlayPause));
            let _ = gs.register(Shortcut::new(None, Code::MediaTrackNext));
            let _ = gs.register(Shortcut::new(None, Code::MediaTrackPrevious));
            Ok(())
        })
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
            commands::record_watch,
            commands::get_history,
            commands::clear_history,
            commands::get_thumbnail,
            commands::download_video,
            commands::play_local,
            commands::pip_play,
            commands::search_suggestions,
            commands::like_video,
            commands::add_to_watch_later,
            commands::mini_player,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}