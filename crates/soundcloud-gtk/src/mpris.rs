use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use mpris_server::*;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MprisState {
    pub is_playing: Arc<AtomicBool>,
    pub current_title: Arc<Mutex<String>>,
    pub volume: Arc<Mutex<f64>>,
    pub position_ms: Arc<Mutex<u64>>,
    pub duration_ms: Arc<Mutex<u64>>,
}

impl MprisState {
    pub fn new() -> Self {
        Self {
            is_playing: Arc::new(AtomicBool::new(false)),
            current_title: Arc::new(Mutex::new(String::new())),
            volume: Arc::new(Mutex::new(0.8)),
            position_ms: Arc::new(Mutex::new(0)),
            duration_ms: Arc::new(Mutex::new(0)),
        }
    }
}

pub fn spawn_mpris(state: MprisState, bus_name: &str) {
    let state_clone = state.clone();
    let bus_name = bus_name.to_string();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio rt for mpris");

        rt.block_on(async move {
            let player = match Player::builder(&bus_name)
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

            let s = state_clone.clone();
            player.connect_play(move |_| {
                s.is_playing.store(true, Ordering::SeqCst);
            });

            let s = state_clone.clone();
            player.connect_pause(move |_| {
                s.is_playing.store(false, Ordering::SeqCst);
            });

            let s = state_clone.clone();
            player.connect_play_pause(move |_| {
                let prev = s.is_playing.load(Ordering::SeqCst);
                s.is_playing.store(!prev, Ordering::SeqCst);
            });

            let s = state_clone.clone();
            player.connect_stop(move |_| {
                s.is_playing.store(false, Ordering::SeqCst);
            });

            let _task = player.run();

            tokio::time::sleep(Duration::from_secs(3600)).await;
        });
    });
}
