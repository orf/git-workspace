use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::ThreadId;
use std::time::{Duration, Instant};

pub struct ProgressManager {
    bars: Arc<Mutex<Vec<ProgressBar>>>,
}

impl ProgressManager {
    pub fn new(multi_bar: &MultiProgress, threads: usize) -> ProgressManager {
        let bars: Vec<ProgressBar> = (0..threads)
            .map(|_| multi_bar.add(ProgressBar::new_spinner()))
            .inspect(|p| p.enable_steady_tick(200))
            .inspect(|p| p.set_message("waiting..."))
            .collect();
        let locked_bars = Arc::new(Mutex::new(bars));

        ProgressManager { bars: locked_bars }
    }
    pub fn create_total_bar(&self, total: u64) -> ProgressBar {
        let bar = ProgressBar::new(total);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {percent}% [{wide_bar:.cyan/blue}] {pos}/{len} (ETA: {eta_precise})")
                .progress_chars("#>-"),
        );
        bar
    }
    pub fn get_bar(&self) -> ProgressBar {
        self.bars.lock().unwrap().pop().unwrap()
    }
    pub fn put_bar(&self, bar: ProgressBar) {
        bar.reset();
        bar.set_message("waiting...");
        self.bars.lock().unwrap().push(bar);
    }
    pub fn signal_done(&self) {
        // Mark all bars as complete
        let bars = self.bars.lock().unwrap();
        for bar in bars.iter() {
            bar.finish();
        }
    }
}
