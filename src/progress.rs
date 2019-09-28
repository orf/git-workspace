use ansi_escapes;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::thread::ThreadId;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub enum Progress {
    Start(String),
    Update(String),
    Finish(Duration),
}

#[derive(Clone)]
pub struct ProgressSender {
    sender: Sender<(ThreadId, Progress)>,
}

impl ProgressSender {
    pub fn new(sender: Sender<(ThreadId, Progress)>) -> ProgressSender {
        ProgressSender { sender }
    }
    pub fn start(&mut self, repo_name: String) -> Instant {
        self.notify(Progress::Start(repo_name));
        Instant::now()
    }
    pub fn update(&self, msg: String) {
        self.notify(Progress::Update(msg));
    }
    pub fn finish(&self, start: Instant) {
        self.notify(Progress::Finish(start.elapsed()))
    }
    fn notify(&self, progress: Progress) {
        self.sender.send((thread::current().id(), progress));
    }
}

pub struct ProgressMonitor {
    receiver: Receiver<(ThreadId, Progress)>,
}

impl ProgressMonitor {
    pub fn new(receiver: Receiver<(ThreadId, Progress)>) -> ProgressMonitor {
        ProgressMonitor { receiver }
    }

    pub fn start(&self) {
        /*
        Here is how this works:
        We receive a status message from each thread, with it's thread ID.
        If the message is a Start message, we push it's thread ID and a message into the stack
        If it's an update we replace the message in the stack with the provided string
        If it's a finish message we write a message about the length of the clone.
        At the end of the message processing we print a summary of all the thread statuses.
        We use an ansi escape code to move the cursor to the start at the beginning of each
        processing, which lets us overwrite the last status update.
        */
        let mut stack: Vec<(ThreadId, String, String)> = vec![];
        for (thread_id, msg) in self.receiver.iter() {
            let start_lines = stack.len() as u16;
            print!("{}", ansi_escapes::EraseLines(start_lines + 1));

            match msg {
                Progress::Start(repo) => {
                    stack.push((thread_id, repo, "".to_string()));
                }
                Progress::Update(msg) => {
                    stack
                        .iter_mut()
                        .filter(|t| t.0 == thread_id)
                        .for_each(|t| t.2 = msg.clone());
                }
                Progress::Finish(duration) => {
                    let position = stack
                        .iter()
                        .position(|(id, _, _)| id == &thread_id)
                        .unwrap();
                    let (_, removed_repo, _) = stack.remove(position);
                    let duration = duration.as_secs();
                    if duration > 30 {
                        println!("{} cloned in {} seconds", removed_repo, duration);
                    }
                }
            }

            for (_, repo, msg) in stack.iter() {
                println!("{}: {}", repo, msg);
            }
        }
    }
}
