use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::thread::ThreadId;

#[derive(Clone)]
pub struct ProgressSender {
    sender: Sender<(ThreadId, String)>,
}

impl ProgressSender {
    pub fn new(sender: Sender<(ThreadId, String)>) -> ProgressSender {
        ProgressSender { sender }
    }
    pub fn notify(&self, message: &String) {
        self.sender
            .send((thread::current().id(), String::from(message)));
    }
}
