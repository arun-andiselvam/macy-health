//! Short UI sounds, played natively: webviews may block audio that isn't
//! triggered by a click. The audio device is opened only while playing.

use std::{
    io::Cursor,
    sync::{mpsc, OnceLock},
    thread,
    time::Duration,
};

use rodio::{Decoder, DeviceSinkBuilder, Source};
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sound {
    Chime,
    Complete,
}

impl Sound {
    fn bytes(self) -> &'static [u8] {
        match self {
            Sound::Chime => include_bytes!("../sounds/chime.wav"),
            Sound::Complete => include_bytes!("../sounds/complete.wav"),
        }
    }
}

static PLAYER: OnceLock<mpsc::Sender<Sound>> = OnceLock::new();

pub fn play(sound: Sound) {
    let sender = PLAYER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Sound>();
        thread::spawn(move || {
            for sound in rx {
                if let Err(err) = play_blocking(sound) {
                    eprintln!("sound: {err}");
                }
            }
        });
        tx
    });
    let _ = sender.send(sound);
}

fn play_blocking(sound: Sound) -> Result<(), Box<dyn std::error::Error>> {
    let source = Decoder::new(Cursor::new(sound.bytes()))?;
    let length = source
        .total_duration()
        .unwrap_or(Duration::from_millis(1500));
    let mut sink = DeviceSinkBuilder::open_default_sink()?;
    sink.log_on_drop(false);
    sink.mixer().add(source);
    // Keep the device open until the sound has finished.
    thread::sleep(length + Duration::from_millis(100));
    Ok(())
}

#[tauri::command]
pub fn play_sound(sound: Sound) {
    play(sound);
}
