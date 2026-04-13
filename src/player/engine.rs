use anyhow::{Context, Result};
use rodio::{Decoder, OutputStream, Sink};
use std::io::Cursor;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum PlayerCommand {
    Play(Vec<u8>),
    Pause,
    Resume,
    Seek(std::time::Duration),
    Shutdown,
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
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl AudioEngine {
    pub fn new() -> Result<Self> {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let thread_handle = std::thread::spawn(move || {
            audio_thread(cmd_rx, event_tx);
        });

        Ok(Self {
            cmd_tx,
            event_rx,
            thread_handle: Some(thread_handle),
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

    pub fn seek(&self, pos: std::time::Duration) -> Result<()> {
        self.cmd_tx.send(PlayerCommand::Seek(pos))?;
        Ok(())
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(PlayerCommand::Shutdown);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

fn audio_thread(
    mut cmd_rx: mpsc::UnboundedReceiver<PlayerCommand>,
    event_tx: mpsc::UnboundedSender<PlayerEvent>,
) {
    let Ok(stream) = rodio::OutputStreamBuilder::open_default_stream() else {
        let _ = event_tx.send(PlayerEvent::Error("Failed to open audio output".into()));
        return;
    };

    let mut current_sink: Option<Sink> = None;

    loop {
        if let Some(ref sink) = current_sink
            && sink.empty()
        {
            current_sink = None;
            let _ = event_tx.send(PlayerEvent::Finished);
        }

        match cmd_rx.try_recv() {
            Ok(cmd) => match cmd {
                PlayerCommand::Play(data) => {
                    if let Some(ref sink) = current_sink {
                        sink.stop();
                    }
                    match play_mp3(&stream, &data) {
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
                PlayerCommand::Seek(pos) => {
                    if let Some(ref sink) = current_sink {
                        let _ = sink.try_seek(pos);
                    }
                }
                PlayerCommand::Shutdown => {
                    if let Some(sink) = current_sink.take() {
                        sink.stop();
                    }
                    break;
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

fn play_mp3(stream: &OutputStream, data: &[u8]) -> Result<Sink> {
    let len = data.len() as u64;
    let cursor = Cursor::new(data.to_vec());
    let source = Decoder::builder()
        .with_data(cursor)
        .with_byte_len(len)
        .with_seekable(true)
        .build()
        .context("Failed to decode audio data")?;
    let sink = rodio::Sink::connect_new(stream.mixer());
    sink.append(source);
    Ok(sink)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_flac_seek_in_memory() {
        let flac_data = include_bytes!("../../tests/fixtures/test.flac");

        // Initialise audio just to get the stream handle (required for Sink)
        let stream = rodio::OutputStreamBuilder::open_default_stream().unwrap();

        // Attempt decoding and seeking via the player's stream load mechanism
        let sink = play_mp3(&stream, flac_data).expect("Failed to play FLAC");

        // Attempt to seek to 0.5s into the track stream
        sink.try_seek(Duration::from_millis(500))
            .expect("Failed to seek FLAC cursor");
    }
}
