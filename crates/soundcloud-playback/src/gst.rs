use std::sync::Arc;
use std::sync::atomic;
use std::time::Duration;

use gstreamer::prelude::*;
use gstreamer::{ClockTime, Element, ElementFactory, SeekFlags, State};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GstError {
    #[error("GStreamer initialization failed")]
    Init,
    #[error("failed to create playbin element")]
    Playbin,
    #[error("state change failed")]
    StateChange,
    #[error("seek not supported")]
    SeekNotSupported,
}

/// GStreamer-based audio playback backend using playbin.
pub struct GstBackend {
    playbin: Element,
    playing: Arc<atomic::AtomicBool>,
}

impl GstBackend {
    /// Initialise GStreamer (idempotent) and create a playbin pipeline.
    pub fn new() -> Result<Self, GstError> {
        gstreamer::init().map_err(|_| GstError::Init)?;

        let playbin = ElementFactory::make("playbin")
            .name("playbin")
            .build()
            .map_err(|_| GstError::Playbin)?;

        let playing = Arc::new(atomic::AtomicBool::new(false));

        Ok(Self { playbin, playing })
    }

    /// Set the URI to play.
    pub fn set_uri(&self, uri: &str) -> Result<(), GstError> {
        self.playbin.set_property_from_str("uri", uri);
        Ok(())
    }

    /// Start or resume playback.
    pub fn play(&self) -> Result<(), GstError> {
        self.set_state(State::Playing)?;
        self.playing.store(true, atomic::Ordering::SeqCst);
        Ok(())
    }

    /// Pause playback (keeps position).
    pub fn pause(&self) -> Result<(), GstError> {
        self.set_state(State::Paused)?;
        self.playing.store(false, atomic::Ordering::SeqCst);
        Ok(())
    }

    /// Stop playback and reset position.
    pub fn stop(&self) -> Result<(), GstError> {
        self.set_state(State::Null)?;
        self.playing.store(false, atomic::Ordering::SeqCst);
        Ok(())
    }

    /// Seek to a position.
    pub fn seek(&self, position: Duration) -> Result<(), GstError> {
        let ns = position.as_nanos();
        let clock = ClockTime::from_nseconds(ns as u64);
        self.playbin
            .seek_simple(SeekFlags::FLUSH | SeekFlags::KEY_UNIT, clock)
            .map_err(|_| GstError::SeekNotSupported)
    }

    /// Current playback position.
    pub fn position(&self) -> Option<Duration> {
        let pos = self.playbin.query_position::<ClockTime>()?;
        let ns = pos.nseconds();
        if ns == 0 {
            return None;
        }
        Some(Duration::from_nanos(ns))
    }

    /// Total duration of the current track.
    pub fn duration(&self) -> Option<Duration> {
        let dur = self.playbin.query_duration::<ClockTime>()?;
        let ns = dur.nseconds();
        if ns == 0 {
            return None;
        }
        Some(Duration::from_nanos(ns))
    }

    /// True when playback is actively playing (not paused/stopped).
    pub fn is_playing(&self) -> bool {
        self.playing.load(atomic::Ordering::SeqCst)
    }

    /// Set volume (0.0 = silent, 1.0 = normal).
    pub fn set_volume(&self, vol: f64) {
        self.playbin
            .set_property_from_str("volume", &vol.to_string());
    }

    fn set_state(&self, state: State) -> Result<(), GstError> {
        self.playbin
            .set_state(state)
            .map_err(|_| GstError::StateChange)?;
        Ok(())
    }
}

impl Drop for GstBackend {
    fn drop(&mut self) {
        let _ = self.playbin.set_state(State::Null);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gst_backend_constructs() {
        let backend = GstBackend::new();
        assert!(backend.is_ok());
    }

    #[test]
    fn set_uri_accepts_valid_uri() {
        let backend = GstBackend::new().unwrap();
        backend.set_uri("https://example.com/audio.mp3").unwrap();
    }

    #[test]
    fn play_pause_cycle_does_not_panic() {
        let backend = GstBackend::new().unwrap();
        backend.set_uri("file:///dev/null").ok();
        let _ = backend.play();
        let _ = backend.pause();
        let _ = backend.stop();
    }
}
