use anyhow::{Context, Result};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::io::Cursor;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum PlayerCommand {
    Play(Vec<u8>), // MP3 bytes to play
    Pause,
    Resume,
    SetVolume(f32), // 0.0 - 1.0
}

#[derive(Debug, Clone)]
pub enum PlayerEvent {
    Started,
    Finished,
    Paused,
    Resumed,
    Error(String),
}

pub struct AudioEngine {
    cmd_tx: mpsc::UnboundedSender<PlayerCommand>,
    pub event_rx: mpsc::UnboundedReceiver<PlayerEvent>,
    volume: f32,
}

impl AudioEngine {
    pub fn new() -> Result<Self> {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        std::thread::spawn(move || {
            audio_thread(cmd_rx, event_tx);
        });

        Ok(Self {
            cmd_tx,
            event_rx,
            volume: 0.8,
        })
    }

    pub fn play(&self, mp3_data: Vec<u8>) -> Result<()> {
        self.cmd_tx.send(PlayerCommand::Play(mp3_data))?;
        Ok(())
    }

    pub fn pause(&self) -> Result<()> {
        self.cmd_tx.send(PlayerCommand::Pause)?;
        Ok(())
    }

    pub fn resume(&self) -> Result<()> {
        self.cmd_tx.send(PlayerCommand::Resume)?;
        Ok(())
    }

    pub fn set_volume(&mut self, vol: f32) -> Result<()> {
        self.volume = vol.clamp(0.0, 1.0);
        self.cmd_tx.send(PlayerCommand::SetVolume(self.volume))?;
        Ok(())
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn volume_up(&mut self) -> Result<()> {
        self.set_volume(self.volume + 0.05)
    }

    pub fn volume_down(&mut self) -> Result<()> {
        self.set_volume(self.volume - 0.05)
    }
}

fn audio_thread(
    mut cmd_rx: mpsc::UnboundedReceiver<PlayerCommand>,
    event_tx: mpsc::UnboundedSender<PlayerEvent>,
) {
    let Ok((_stream, stream_handle)) = OutputStream::try_default() else {
        let _ = event_tx.send(PlayerEvent::Error("Failed to open audio output".into()));
        return;
    };

    let mut current_sink: Option<Sink> = None;

    // We need a runtime for the blocking recv
    loop {
        // Check if current track finished
        if let Some(ref sink) = current_sink {
            if sink.empty() {
                current_sink = None;
                let _ = event_tx.send(PlayerEvent::Finished);
            }
        }

        // Non-blocking check for commands
        match cmd_rx.try_recv() {
            Ok(cmd) => match cmd {
                PlayerCommand::Play(data) => {
                    // Stop current playback
                    if let Some(ref sink) = current_sink {
                        sink.stop();
                    }
                    match play_mp3(&stream_handle, &data) {
                        Ok(sink) => {
                            current_sink = Some(sink);
                            let _ = event_tx.send(PlayerEvent::Started);
                        }
                        Err(e) => {
                            let _ = event_tx.send(PlayerEvent::Error(e.to_string()));
                        }
                    }
                }
                PlayerCommand::Pause => {
                    if let Some(ref sink) = current_sink {
                        sink.pause();
                        let _ = event_tx.send(PlayerEvent::Paused);
                    }
                }
                PlayerCommand::Resume => {
                    if let Some(ref sink) = current_sink {
                        sink.play();
                        let _ = event_tx.send(PlayerEvent::Resumed);
                    }
                }
                PlayerCommand::SetVolume(vol) => {
                    if let Some(ref sink) = current_sink {
                        sink.set_volume(vol);
                    }
                }
            },
            Err(mpsc::error::TryRecvError::Empty) => {
                // No commands, sleep briefly to avoid busy-waiting
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(mpsc::error::TryRecvError::Disconnected) => {
                // Channel closed, exit thread
                break;
            }
        }
    }
}

fn play_mp3(stream_handle: &OutputStreamHandle, data: &[u8]) -> Result<Sink> {
    let cursor = Cursor::new(data.to_vec());
    let source = Decoder::new(cursor).context("Failed to decode MP3 data")?;
    let sink = Sink::try_new(stream_handle).context("Failed to create audio sink")?;
    sink.append(source);
    Ok(sink)
}
