//! Música tocando agora (Spotify, navegadores, qualquer app que use os controles de mídia do Windows).
//! Lê numa thread própria a cada meio segundo; a interface só consulta o último estado.

use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaState {
    pub active: bool,
    pub title: String,
    pub artist: String,
    pub app: String,
    pub playing: bool,
    pub position: f64,
    pub duration: f64,
}

#[derive(Clone, Copy, Debug)]
pub enum MediaCmd { Toggle, Next, Prev }

pub struct Media {
    pub state: Arc<Mutex<MediaState>>,
    tx: std::sync::mpsc::Sender<MediaCmd>,
}

impl Media {
    pub fn start(repaint: impl Fn() + Send + 'static) -> Self {
        let state = Arc::new(Mutex::new(MediaState::default()));
        let (tx, rx) = std::sync::mpsc::channel::<MediaCmd>();
        let st = state.clone();
        std::thread::spawn(move || imp::run(st, rx, repaint));
        Media { state, tx }
    }
    pub fn snapshot(&self) -> MediaState { self.state.lock().map(|s| s.clone()).unwrap_or_default() }
    pub fn send(&self, c: MediaCmd) { let _ = self.tx.send(c); }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::Media::Control::{GlobalSystemMediaTransportControlsSessionManager as Mgr, GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status};

    pub fn run(st: Arc<Mutex<MediaState>>, rx: std::sync::mpsc::Receiver<MediaCmd>, repaint: impl Fn()) {
        let mgr = loop {
            match Mgr::RequestAsync().and_then(|op| op.get()) {
                Ok(m) => break m,
                Err(_) => std::thread::sleep(std::time::Duration::from_secs(5)),
            }
        };
        loop {
            let mut next = MediaState::default();
            if let Ok(s) = mgr.GetCurrentSession() {
                // comandos pendentes vão para a sessão atual
                while let Ok(c) = rx.try_recv() {
                    let _ = match c {
                        MediaCmd::Toggle => s.TryTogglePlayPauseAsync().map(|_| ()),
                        MediaCmd::Next => s.TrySkipNextAsync().map(|_| ()),
                        MediaCmd::Prev => s.TrySkipPreviousAsync().map(|_| ()),
                    };
                }
                if let Ok(p) = s.TryGetMediaPropertiesAsync().and_then(|op| op.get()) {
                    next.title = p.Title().map(|h| h.to_string()).unwrap_or_default();
                    next.artist = p.Artist().map(|h| h.to_string()).unwrap_or_default();
                }
                next.app = s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
                if let Ok(info) = s.GetPlaybackInfo() {
                    next.playing = info.PlaybackStatus().map(|x| x == Status::Playing).unwrap_or(false);
                }
                if let Ok(tl) = s.GetTimelineProperties() {
                    let ticks = |d: windows::Foundation::TimeSpan| d.Duration as f64 / 10_000_000.0;
                    next.position = tl.Position().map(ticks).unwrap_or(0.0);
                    next.duration = tl.EndTime().map(ticks).unwrap_or(0.0);
                }
                next.active = !next.title.is_empty();
            } else {
                while rx.try_recv().is_ok() {}
            }
            let changed = { let mut g = st.lock().unwrap(); let c = *g != next; *g = next; c };
            if changed { repaint(); }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn run(st: Arc<Mutex<MediaState>>, rx: std::sync::mpsc::Receiver<MediaCmd>, repaint: impl Fn()) {
        // modo demonstração fora do Windows
        { let mut g = st.lock().unwrap(); *g = MediaState { active: true, title: "Neon Drive".into(), artist: "Pixel Kids".into(), app: "Demo".into(), playing: false, position: 44.0, duration: 195.0 }; }
        repaint();
        for c in rx { let mut g = st.lock().unwrap(); if let MediaCmd::Toggle = c { g.playing = !g.playing; } repaint(); }
    }
}
