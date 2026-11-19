import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type Video = {
  id: string;
  title: string;
  channel_id?: string | null;
  channel_title?: string | null;
  description?: string | null;
  thumbnail?: string | null;
  duration?: string | null;
  published_at?: string | null;
  view_count?: string | null;
};

type Sub = { id: string; title?: string | null; thumbnail?: string | null; channelId?: string | null };

export default function App() {
  const [view, setView] = useState<"search" | "watch" | "channel" | "subs">("search");
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Video[]>([]);
  const [nextToken, setNextToken] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [watchId, setWatchId] = useState<string | null>(null);
  const [watchMeta, setWatchMeta] = useState<Video | null>(null);
  const [comments, setComments] = useState<any[]>([]);
  const [related, setRelated] = useState<Video[]>([]);
  const [channelData, setChannelData] = useState<any>(null);
  const [channelVideos, setChannelVideos] = useState<Video[]>([]);
  const [subs, setSubs] = useState<Sub[]>([]);
  const [authed, setAuthed] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [mpvAvailable, setMpvAvailable] = useState(false);
  const [settings, setSettings] = useState({ api_key: "", client_id: "", client_secret: "" });

  useEffect(() => {
    invoke<any>("auth_status").then((r) => setAuthed(!!r.authenticated)).catch(() => {});
    invoke<any>("get_settings").then((r) => {
      setSettings({ api_key: r.api_key || "", client_id: r.client_id || "", client_secret: r.client_secret || "" });
      if (!r.has_api_key) setShowSettings(true);
    }).catch(() => {});
    invoke<boolean>("check_mpv").then((r) => setMpvAvailable(!!r)).catch(() => setMpvAvailable(false));
  }, []);

  async function search(q?: string, token?: string | null) {
    const qq = q ?? query;
    if (!qq.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const r: any = await invoke("search_videos", { query: qq, pageToken: token ?? null });
      const items = (r.items || []) as Video[];
      if (token) setResults((prev) => [...prev, ...items]);
      else setResults(items);
      setNextToken(r.nextPageToken ?? null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function openVideo(id: string) {
    setWatchId(id);
    setView("watch");
    setComments([]);
    setRelated([]);
    try {
      const v: any = await invoke("get_video", { id });
      setWatchMeta(v as Video);
    } catch (e) {
      setError(String(e));
    }
    invoke<any>("get_comments", { videoId: id }).then((r) => setComments(r.items || [])).catch(() => {});
    invoke<any>("get_related", { videoId: id }).then((r) => setRelated((r.items || []) as Video[])).catch(() => {});
  }

  async function openChannel(id: string) {
    setView("channel");
    setChannelData(null);
    setChannelVideos([]);
    try {
      const c: any = await invoke("get_channel", { id });
      setChannelData(c);
      const feed: any = await invoke("get_subscription_feed", { channelId: id }).catch(() => null);
      // fallback: try uploads via playlist items not exposed; use search by channel
      if (feed && feed.items) setChannelVideos(feed.items as Video[]);
    } catch (e) {
      setError(String(e));
    }
  }

  async function loadSubs() {
    setView("subs");
    setLoading(true);
    setError(null);
    try {
      const r: any = await invoke("get_subscriptions", { pageToken: null });
      setSubs((r.items || []) as Sub[]);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function doLogin() {
    try {
      await invoke("auth_login");
      setAuthed(true);
      await loadSubs();
    } catch (e) {
      setError(String(e));
    }
  }

  async function doLogout() {
    await invoke("auth_logout");
    setAuthed(false);
    setSubs([]);
  }

  function fmtDuration(d?: string | null) {
    if (!d) return "";
    // PT#H#M#S
    const m = d.match(/PT(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?/);
    if (!m) return d;
    const h = m[1] ? m[1] + ":" : "";
    const mm = (m[2] || "0").padStart(h ? 2 : 1, "0");
    const ss = (m[3] || "0").padStart(2, "0");
    return `${h}${mm}:${ss}`;
  }

  return (
    <div className="app">
      <header className="topbar">
        <button className="logo" onClick={() => setView("search")}>YT</button>
        <input
          className="searchbox"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter") search(); }}
          placeholder="Search YouTube..."
        />
        <button onClick={() => search()}>Search</button>
        <button onClick={() => (authed ? loadSubs() : doLogin())}>
          {authed ? "Subscriptions" : "Sign in"}
        </button>
        {authed && <button onClick={doLogout}>Sign out</button>}
        <button onClick={() => setShowSettings(true)}>Settings</button>
      </header>

      {showSettings && (
        <div className="modal-backdrop" onClick={() => setShowSettings(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h2>Settings</h2>
            <p className="hint">
              Create credentials in Google Cloud Console (YouTube Data API v3 enabled, OAuth "Desktop app").
              <br />
              <a href="https://console.cloud.google.com/apis/credentials" target="_blank" rel="noreferrer">Open Google Cloud Console</a>
            </p>
            <label>API Key</label>
            <input value={settings.api_key} onChange={(e) => setSettings({ ...settings, api_key: e.target.value })} placeholder="AIza..." />
            <label>OAuth Client ID</label>
            <input value={settings.client_id} onChange={(e) => setSettings({ ...settings, client_id: e.target.value })} placeholder="....apps.googleusercontent.com" />
            <label>OAuth Client Secret (optional)</label>
            <input value={settings.client_secret} onChange={(e) => setSettings({ ...settings, client_secret: e.target.value })} placeholder="GOCSPX-..." />
            <div className="modal-actions">
              <button onClick={async () => {
                await invoke("set_settings", { apiKey: settings.api_key, clientId: settings.client_id, clientSecret: settings.client_secret });
                setShowSettings(false);
              }}>Save</button>
              <button onClick={() => setShowSettings(false)}>Cancel</button>
            </div>
          </div>
        </div>
      )}

      {error && <div className="error">{error}</div>}
      {loading && <div className="loading">Loading...</div>}

      {view === "search" && (
        <div className="grid">
          {results.map((v) => (
            <div key={v.id} className="card" onClick={() => openVideo(v.id)}>
              {v.thumbnail && <img src={v.thumbnail} alt="" />}
              <div className="cardtitle">{v.title}</div>
              <div className="cardmeta">{v.channel_title} · {fmtDuration(v.duration)}</div>
            </div>
          ))}
          {nextToken && (
            <button className="more" onClick={() => search(query, nextToken)}>Load more</button>
          )}
        </div>
      )}

      {view === "watch" && watchId && (
        <div className="watch">
          <div className="nativebar">
            {mpvAvailable ? (
              <button className="playnative" onClick={async () => {
                try { await invoke("play_in_mpv", { videoId: watchId }); }
                catch (e) { setError(String(e)); }
              }}>Play in mpv (native, independent)</button>
            ) : (
              <span className="hint">mpv not installed — using embedded player. Install mpv for native playback.</span>
            )}
          </div>
          <div className="player">
            <iframe
              src={`https://www.youtube.com/embed/${watchId}?autoplay=1&enablejsapi=1`}
              title="player"
              allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
              allowFullScreen
            />
          </div>
          {watchMeta && (
            <div className="meta">
              <h2>{watchMeta.title}</h2>
              <p className="channel" onClick={() => watchMeta.channel_id && openChannel(watchMeta.channel_id)}>
                {watchMeta.channel_title}
              </p>
              <div className="stats">{watchMeta.view_count} views · {fmtDuration(watchMeta.duration)}</div>
              <p className="desc">{watchMeta.description}</p>
            </div>
          )}

          <div className="watchbody">
            <div className="comments">
              <h3>Comments</h3>
              {comments.length === 0 && <p className="hint">No comments or loading...</p>}
              {comments.map((c: any) => (
                <div key={c.id} className="comment">
                  {c.avatar && <img src={c.avatar} alt="" />}
                  <div>
                    <div className="commentauthor">{c.author} <span className="commentdate">{c.publishedAt?.slice(0, 10)}</span></div>
                    <div className="commenttext">{c.text}</div>
                    {c.likes ? <div className="commentlikes">👍 {c.likes}</div> : null}
                  </div>
                </div>
              ))}
            </div>
            <div className="related">
              <h3>Related</h3>
              {related.map((v) => (
                <div key={v.id} className="relatedcard" onClick={() => openVideo(v.id)}>
                  {v.thumbnail && <img src={v.thumbnail} alt="" />}
                  <div>
                    <div className="cardtitle">{v.title}</div>
                    <div className="cardmeta">{v.channel_title}</div>
                  </div>
                </div>
              ))}
            </div>
          </div>

          <button onClick={() => setView("search")}>Back</button>
        </div>
      )}

      {view === "channel" && (
        <div className="channel">
          {channelData && (
            <>
              <div className="cheader">
                {channelData.thumbnail && <img src={channelData.thumbnail} alt="" />}
                <div>
                  <h2>{channelData.title}</h2>
                  <p>{channelData.subscriberCount} subscribers · {channelData.videoCount} videos</p>
                </div>
              </div>
              <p className="desc">{channelData.description}</p>
            </>
          )}
          <div className="grid">
            {channelVideos.map((v) => (
              <div key={v.id} className="card" onClick={() => openVideo(v.id)}>
                {v.thumbnail && <img src={v.thumbnail} alt="" />}
                <div className="cardtitle">{v.title}</div>
                <div className="cardmeta">{fmtDuration(v.duration)}</div>
              </div>
            ))}
          </div>
          <button onClick={() => setView("search")}>Back</button>
        </div>
      )}

      {view === "subs" && (
        <div className="subs">
          <h2>Subscriptions</h2>
          <div className="sublist">
            {subs.map((s) => (
              <div key={s.id} className="subitem" onClick={() => s.channelId && openChannel(s.channelId)}>
                {s.thumbnail && <img src={s.thumbnail} alt="" />}
                <span>{s.title}</span>
              </div>
            ))}
          </div>
          <button onClick={() => setView("search")}>Back</button>
        </div>
      )}
    </div>
  );
}