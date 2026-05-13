use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use mpris_server::*;

#[allow(dead_code)]
#[derive(Clone)]
pub struct MprisState {
    pub is_playing: Arc<AtomicBool>,
    pub current_title: Arc<Mutex<String>>,
    pub volume: Arc<Mutex<f64>>,
    pub position_ms: Arc<Mutex<u64>>,
    pub duration_ms: Arc<Mutex<u64>>,
    /// Set by MPRIS callback, polled by GTK loop
    pub pending_play: Arc<AtomicBool>,
    pub pending_pause: Arc<AtomicBool>,
    pub pending_play_pause: Arc<AtomicBool>,
    pub pending_stop: Arc<AtomicBool>,
    pub pending_next: Arc<AtomicBool>,
    pub pending_previous: Arc<AtomicBool>,
}

impl MprisState {
    pub fn new() -> Self {
        Self {
            is_playing: Arc::new(AtomicBool::new(false)),
            current_title: Arc::new(Mutex::new(String::new())),
            volume: Arc::new(Mutex::new(0.8)),
            position_ms: Arc::new(Mutex::new(0)),
            duration_ms: Arc::new(Mutex::new(0)),
            pending_play: Arc::new(AtomicBool::new(false)),
            pending_pause: Arc::new(AtomicBool::new(false)),
            pending_play_pause: Arc::new(AtomicBool::new(false)),
            pending_stop: Arc::new(AtomicBool::new(false)),
            pending_next: Arc::new(AtomicBool::new(false)),
            pending_previous: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn consume_play(&self) -> bool {
        self.pending_play.swap(false, Ordering::SeqCst)
    }
    pub fn consume_pause(&self) -> bool {
        self.pending_pause.swap(false, Ordering::SeqCst)
    }
    pub fn consume_play_pause(&self) -> bool {
        self.pending_play_pause.swap(false, Ordering::SeqCst)
    }
    pub fn consume_stop(&self) -> bool {
        self.pending_stop.swap(false, Ordering::SeqCst)
    }
    #[allow(dead_code)]
    pub fn consume_next(&self) -> bool {
        self.pending_next.swap(false, Ordering::SeqCst)
    }
    #[allow(dead_code)]
    pub fn consume_previous(&self) -> bool {
        self.pending_previous.swap(false, Ordering::SeqCst)
    }
}

pub fn spawn_mpris(state: MprisState, bus_name: &str) {
    let s = state.clone();
    let name = bus_name.to_string();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio rt for mpris");

        rt.block_on(async move {
            let player = match Player::builder(&name)
                .can_play(true)
                .can_pause(true)
                .can_go_next(true)
                .can_go_previous(true)
                .can_seek(true)
                .can_control(true)
                .build()
                .await
            {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("MPRIS init failed: {e}");
                    return;
                }
            };

            let s2 = s.clone();
            player.connect_play(move |_| {
                s2.pending_play.store(true, Ordering::SeqCst);
            });

            let s2 = s.clone();
            player.connect_pause(move |_| {
                s2.pending_pause.store(true, Ordering::SeqCst);
            });

            let s2 = s.clone();
            player.connect_play_pause(move |_| {
                s2.pending_play_pause.store(true, Ordering::SeqCst);
            });

            let s2 = s.clone();
            player.connect_stop(move |_| {
                s2.pending_stop.store(true, Ordering::SeqCst);
            });

            let s2 = s.clone();
            player.connect_next(move |_| {
                s2.pending_next.store(true, Ordering::SeqCst);
            });

            let s2 = s.clone();
            player.connect_previous(move |_| {
                s2.pending_previous.store(true, Ordering::SeqCst);
            });

            let _task = player.run();
            tokio::time::sleep(Duration::from_secs(3600)).await;
        });
    });
}
