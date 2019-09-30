use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::{Arc, Mutex};

pub struct ProgressManager {
    bars: Arc<Mutex<Vec<ProgressBar>>>,
}

impl ProgressManager {
    pub fn new(multi_bar: &MultiProgress, threads: usize) -> ProgressManager {
        let bars: Vec<ProgressBar> = (0..threads)
            .map(|_| multi_bar.add(ProgressBar::new_spinner()))
            .inspect(|p| p.enable_steady_tick(400))
            .inspect(|p| p.set_message("waiting..."))
            .collect();
        let locked_bars = Arc::new(Mutex::new(bars));

        ProgressManager { bars: locked_bars }
    }
    pub fn create_total_bar(&self, total: u64) -> ProgressBar {
        let progress_bar = ProgressBar::new(total);
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {percent}% [{wide_bar:.cyan/blue}] {pos}/{len} (ETA: {eta_precise})")
                .progress_chars("#>-"),
        );
        progress_bar
    }
    pub fn get_bar(&self) -> ProgressBar {
        let progress_bar = self.bars.lock().unwrap().pop().unwrap();
        progress_bar.set_message("starting");
        progress_bar
    }
    pub fn put_bar(&self, progress_bar: ProgressBar) {
        progress_bar.reset();
        progress_bar.set_message("waiting...");
        self.bars.lock().unwrap().push(progress_bar);
    }
    pub fn signal_done(&self) {
        // Mark all bars as complete
        let bars = self.bars.lock().unwrap();
        for progress_bar in bars.iter() {
            progress_bar.finish();
        }
    }
}
