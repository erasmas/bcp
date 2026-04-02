use anyhow::{Context, Result};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::io::Cursor;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum PlayerCommand {
    Play(Vec<u8>),
    Pause,
    Resume,
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
}

impl AudioEngine {
    pub fn new() -> Result<Self> {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        std::thread::spawn(move || {
            audio_thread(cmd_rx, event_tx);
        });

        Ok(Self { cmd_tx, event_rx })
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

    loop {
        if let Some(ref sink) = current_sink {
            if sink.empty() {
                current_sink = None;
                let _ = event_tx.send(PlayerEvent::Finished);
            }
        }

        match cmd_rx.try_recv() {
            Ok(cmd) => match cmd {
                PlayerCommand::Play(data) => {
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
            },
            Err(mpsc::error::TryRecvError::Empty) => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(mpsc::error::TryRecvError::Disconnected) => {
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
