use std::sync::mpsc::{Sender, Receiver};
use std::thread::ThreadId;
use std::thread;

#[derive(Clone)]
pub struct ProgressSender {
    sender: Sender<(ThreadId, String)>,
}

impl ProgressSender {
    pub fn new(sender: Sender<(ThreadId, String)>) -> ProgressSender {
        ProgressSender {
            sender
        }
    }
    pub fn notify(&self, message: &String) {
        self.sender.send((thread::current().id(), String::from(message)));
    }
}
