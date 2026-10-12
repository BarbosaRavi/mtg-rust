use eframe::egui;
use rust_mtg::collection;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

const SYNC_INTERVAL: Duration = Duration::from_secs(30);
const RETRY_INTERVAL: Duration = Duration::from_secs(5);

pub enum SyncMessage {
    Synced { changed: bool },
    Failed(String),
}

pub struct CollectionSync {
    receiver: Receiver<SyncMessage>,
    wake: Sender<()>,
}

impl CollectionSync {
    pub fn start(ctx: egui::Context, card_db: PathBuf) -> Self {
        let (sender, receiver) = mpsc::channel();
        let (wake, wake_signal) = mpsc::channel();
        thread::spawn(move || worker(&ctx, &sender, &wake_signal, &card_db));

        Self { receiver, wake }
    }

    pub fn messages(&self) -> Vec<SyncMessage> {
        self.receiver.try_iter().collect()
    }

    pub fn sync_now(&self) {
        let _ = self.wake.send(());
    }
}

fn worker(
    ctx: &egui::Context,
    sender: &Sender<SyncMessage>,
    wake_signal: &Receiver<()>,
    card_db: &Path,
) {
    loop {
        let (message, wait) = match collection::sync_from_game(card_db) {
            Ok(outcome) => (
                SyncMessage::Synced {
                    changed: outcome.changed,
                },
                SYNC_INTERVAL,
            ),
            Err(error) => (SyncMessage::Failed(format!("{error:#}")), RETRY_INTERVAL),
        };

        if sender.send(message).is_err() {
            return;
        }
        ctx.request_repaint();

        if let Err(RecvTimeoutError::Disconnected) = wake_signal.recv_timeout(wait) {
            return;
        }
        while wake_signal.try_recv().is_ok() {}
    }
}
