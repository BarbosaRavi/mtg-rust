use eframe::egui;
use rust_mtg::{image_cache, scryfall};
use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

const WORKERS: usize = 4;

pub struct Images {
    requested: HashSet<String>,
    queue: Sender<String>,
}

impl Images {
    pub fn new(ctx: egui::Context) -> Self {
        let (queue, jobs) = mpsc::channel::<String>();
        let jobs = Arc::new(Mutex::new(jobs));

        for _ in 0..WORKERS {
            let jobs = Arc::clone(&jobs);
            let ctx = ctx.clone();
            thread::spawn(move || download_worker(&jobs, &ctx));
        }

        Self {
            requested: HashSet::new(),
            queue,
        }
    }

    pub fn uri(&mut self, url: &str) -> Option<String> {
        let path = image_cache::cached_path(url).ok()?;
        if path.is_file() {
            return Some(image_cache::file_uri(&path));
        }

        if self.requested.insert(url.to_string()) {
            let _ = self.queue.send(url.to_string());
        }
        None
    }
}

fn download_worker(jobs: &Mutex<Receiver<String>>, ctx: &egui::Context) {
    let Ok(client) = scryfall::client() else {
        return;
    };

    loop {
        let next = match jobs.lock() {
            Ok(receiver) => receiver.recv(),
            Err(_) => return,
        };
        let Ok(url) = next else {
            return;
        };

        match image_cache::cached_path(&url) {
            Ok(path) if !path.is_file() => {
                if let Err(error) = image_cache::download(&client, &url, &path) {
                    eprintln!("aviso: {error:#}");
                }
            }
            Ok(_) => {}
            Err(error) => eprintln!("aviso: {error:#}"),
        }
        ctx.request_repaint();
    }
}
