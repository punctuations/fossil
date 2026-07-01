use std::io::{self, IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use crate::utils::color::paint;

pub struct Spinner {
    done: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Spinner {
    pub fn start(msg: &str) -> Self {
        Self::colored(msg, "38;5;173")
    }

    pub fn dim(msg: &str) -> Self {
        Self::colored(msg, "38;5;244")
    }

    fn colored(msg: &str, code: &str) -> Self {
        let done = Arc::new(AtomicBool::new(false));

        if !io::stderr().is_terminal() {
            return Spinner { done, handle: None };
        }

        let flag = done.clone();
        let msg = msg.to_string();
        let code = code.to_string();
        let handle = thread::spawn(move || {
            let frames = [
                "⡀⠀⠀",
                "⡄⠀⠀",
                "⡆⠀⠀",
                "⡇⠀⠀",
                "⣇⠀⠀",
                "⣧⠀⠀",
                "⣷⠀⠀",
                "⣿⠀⠀",
                "⣿⡀⠀",
                "⣿⡄⠀",
                "⣿⡆⠀",
                "⣿⡇⠀",
                "⣿⣇⠀",
                "⣿⣧⠀",
                "⣿⣷⠀",
                "⣿⣿⠀",
                "⣿⣿⡀",
                "⣿⣿⡄",
                "⣿⣿⡆",
                "⣿⣿⡇",
                "⣿⣿⣇",
                "⣿⣿⣧",
                "⣿⣿⣷",
                "⣿⣿⣿",
                "⣿⣿⣿",
                "⣿⣿⣿",
                "⣿⣿⣿",
                "⣿⣿⣷",
                "⣿⣿⣧",
                "⣿⣿⣇",
                "⣿⣿⡆",
                "⣿⣿⡄",
                "⣿⣿⡀",
                "⣿⣿⠀",
                "⣿⣷⠀",
                "⣿⣧⠀",
                "⣿⣇⠀",
                "⣿⡇⠀",
                "⣿⡆⠀",
                "⣿⡄⠀",
                "⣿⡀⠀",
                "⣿⠀⠀",
                "⣷⠀⠀",
                "⣧⠀⠀",
                "⣇⠀⠀",
                "⡇⠀⠀",
                "⡆⠀⠀",
                "⡄⠀⠀",
                "⡀⠀⠀",
                "⠀⠀⠀",
            ];
            let mut i = 0;
            while !flag.load(Ordering::Relaxed) {
                eprint!(
                    "\r  {} {}",
                    paint(frames[i % frames.len()], &code),
                    paint(&msg, &code)
                );
                let _ = io::stderr().flush();
                i += 1;
                thread::sleep(Duration::from_millis(80));
            }
            eprint!("\r\x1b[2K");
            let _ = io::stderr().flush();
        });

        Spinner {
            done,
            handle: Some(handle),
        }
    }

    pub fn stop(self) {
        self.done.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle {
            let _ = h.join();
        }
    }
}
