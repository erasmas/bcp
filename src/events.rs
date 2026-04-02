use crossterm::event::{self, Event, KeyEvent};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    Tick,
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<AppEvent>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        std::thread::spawn(move || {
            loop {
                if event::poll(tick_rate).unwrap_or(false) {
                    if let Ok(Event::Key(key)) = event::read() {
                        if tx.send(AppEvent::Key(key)).is_err() {
                            break;
                        }
                    }
                } else if tx.send(AppEvent::Tick).is_err() {
                    break;
                }
            }
        });

        Self { rx }
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }
}
