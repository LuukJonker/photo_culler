// use std::thread::{self, JoinHandle};

// use crossbeam::channel::{Sender, Receiver};

// use crate::commands::{RenderRequest, Request};

// pub struct RenderThread {
//     sender: Sender<RenderRequest>,
//     receiver: Receiver<RenderRequest>,
// }

// impl RenderThread {
//     pub fn new(sender: Sender<Request>, receiver: Receiver<Request>) -> Self {
//         Self {
//             sender,
//             receiver,
//         }
//     }

//     fn inner(self) {

//     }

//     pub fn run(self) -> JoinHandle<()> {
//         thread::spawn(|| self.inner())
//     }
// }
