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
    /// capa do álbum (bytes PNG/JPEG) e a chave da faixa a que ela pertence
    pub art: Option<Arc<Vec<u8>>>,
    pub art_key: String,
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

    fn read_thumb(r: &windows::Storage::Streams::IRandomAccessStreamReference) -> Option<Vec<u8>> {
        use windows::Storage::Streams::{DataReader, InputStreamOptions};
        let stream = r.OpenReadAsync().ok()?.get().ok()?;
        let size = stream.Size().ok()? as u32;
        if size == 0 || size > 8_000_000 { return None; }
        let reader = DataReader::CreateDataReader(&stream).ok()?;
        reader.SetInputStreamOptions(InputStreamOptions::ReadAhead).ok()?;
        let n = reader.LoadAsync(size).ok()?.get().ok()?;
        let mut buf = vec![0u8; n as usize];
        reader.ReadBytes(&mut buf).ok()?;
        Some(buf)
    }

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
                    let key = format!("{}|{}", next.title, next.artist);
                    // a capa só é lida quando a faixa muda (e de novo se ainda não veio: alguns apps a entregam atrasada)
                    let (prev_key, prev_art) = { let g = st.lock().unwrap(); (g.art_key.clone(), g.art.clone()) };
                    if key == prev_key && prev_art.is_some() { next.art = prev_art; }
                    else if let Ok(th) = p.Thumbnail() { next.art = read_thumb(&th).map(Arc::new); }
                    next.art_key = key;
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
        { let mut g = st.lock().unwrap(); *g = MediaState { active: true, title: "Neon Drive".into(), artist: "Pixel Kids".into(), app: "Demo".into(), playing: false, position: 44.0, duration: 195.0, art: std::env::var("KON_ART").ok().and_then(|p| std::fs::read(p).ok()).map(Arc::new), art_key: "demo".into() }; }
        repaint();
        for c in rx { let mut g = st.lock().unwrap(); if let MediaCmd::Toggle = c { g.playing = !g.playing; } repaint(); }
    }
}
