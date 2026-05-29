pub mod disk_writer;
pub mod image_browser;
pub mod image_container;
mod render_worker;
mod worker;

use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;
use std::{cmp::Ordering, collections::BinaryHeap, thread};

use crate::commands::{Commands, Priority, Request, Response};
use crate::model::image_browser::ImageBrowser;
use crate::model::worker::Worker;
// use crate::model::render_worker::RenderThread;
use crossbeam::channel::{Receiver, Select, Sender, bounded, unbounded};

/// A wrapper around a Request to allow for prioritization in a BinaryHeap.
struct RequestWithPriority {
    /// The request to be processed.
    request: Request,
    /// The priority level of the request.
    priority: Priority,
}

/// Still have to figure out how this works with the eq                                                                                                                             
///                                                                                                                                                                                 
impl Eq for RequestWithPriority {}

impl PartialEq for RequestWithPriority {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl PartialOrd for RequestWithPriority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RequestWithPriority {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority)
    }
}

impl RequestWithPriority {
    /// Creates a new RequestWithPriority from a Request.
    fn new(request: Request) -> Self {
        Self {
            priority: request.priority(),
            request,
        }
    }
}

/// The shared state of the model, accessible by workers.
#[derive(Clone, Default)]
pub struct ModelState {
    /// The image browser instance, wrapped for thread-safe access.
    pub browser: Arc<RwLock<Option<ImageBrowser>>>,
}

/// The core model that manages job scheduling and workers.
pub struct Model {
    // Channel for the incoming job requests from the view model
    incoming_sender: Sender<Request>,
    incoming_receiver: Receiver<Request>,

    // Channel for sending the job requests to the workers
    outgoing_sender: Sender<Request>,
    outgoing_receiver: Receiver<Request>,

    // Channels for sending the job requests to the render worker
    _render_sender: Sender<Request>,
    _render_receiver: Receiver<Request>,

    // Workers, also holds the render worker, so can be different types of workers under need
    worker_handles: Vec<JoinHandle<()>>,

    // Priority queue for the requests
    queue: BinaryHeap<RequestWithPriority>,

    // The state of the model
    state: ModelState,
}

impl Model {
    /// Creates a new Model instance with its internal communication channels.
    pub fn new() -> Self {
        // Incoming and outgoing channels, for dev purposes
        let (incoming_sender, incoming_receiver) = unbounded::<Request>();
        let (outgoing_sender, outgoing_receiver) = bounded::<Request>(0);
        let (_render_sender, _render_receiver) = unbounded::<Request>();

        Self {
            incoming_sender,
            incoming_receiver,
            outgoing_sender,
            outgoing_receiver,
            _render_sender,
            _render_receiver,

            worker_handles: Vec::new(),
            queue: BinaryHeap::new(),

            state: ModelState::default(),
        }
    }

    /// Returns a sender for dispatching requests to the model.
    pub fn get_sender(&self) -> Sender<Request> {
        self.incoming_sender.clone()
    }

    /// Returns a receiver for workers to pull jobs from the model.
    pub fn get_receiver(&self) -> Receiver<Request> {
        self.outgoing_receiver.clone()
    }

    /// The internal event loop of the model manager, handling job prioritization.
    fn inner(&mut self) {
        loop {
            // Rebuild the selector every iteration
            let mut sel = Select::new(); // Can use biased selector to make sure jobs are handed out first

            // 1. Always listen for incoming requests
            let incoming_oper = sel.recv(&self.incoming_receiver);

            // 2. Only attempt to send IF we have something in the queue
            let mut outgoing_oper = None;
            if self.queue.peek().is_some() {
                outgoing_oper = Some(sel.send(&self.outgoing_sender));
            }

            // 3. Block until one of the registered operations is ready
            let oper = sel.select();

            // 4. Handle whichever operation woke up the thread
            match oper.index() {
                // An incoming job arrived
                i if i == incoming_oper => {
                    match oper.recv(&self.incoming_receiver) {
                        Ok(job) => self.queue.push(RequestWithPriority::new(job)),
                        Err(_) => break, // The incoming sender was dropped, safely exit thread
                    }
                }
                // A worker is ready to receive a job
                i if Some(i) == outgoing_oper => {
                    // Safe to unwrap because we only registered this operation if peek().is_some()
                    let job = self.queue.pop().unwrap();

                    if let Commands::KillThread = job.request.command() {
                        // Kill this thread
                        if job.request.priority() == Priority::Low {
                            return;
                        }

                        for _ in 0..11 {
                            self.queue.push(RequestWithPriority {
                                request: Commands::KillThread.critical(),
                                priority: Priority::Critical,
                            });
                        }
                    }
                    // Actually perform the send
                    if oper
                        .send(&self.outgoing_sender, job.request.clone())
                        .is_err()
                    {
                        // The worker disconnected right as we tried to send!
                        // In a robust system, we put the job back in the queue.
                        self.queue.push(job);
                    }
                }
                _ => unreachable!(),
            }
        }
    }

    /// Spawns worker threads and starts the model's management loop.
    pub fn run(mut self, response_sender: Sender<Response>) -> Vec<JoinHandle<()>> {
        let mut worker_handles = Vec::new();

        // Start the worker threads and at only the handles to the
        for _ in 0..10 {
            worker_handles
                .push(Worker::new(self.get_receiver(), response_sender.clone(), &self.state).run());
        }

        // worker_handles.push(RenderThread::new(self.render_sender.clone(), self.render_receiver.clone()).run());

        // Run the manager part of the model
        thread::spawn(move || self.inner());

        worker_handles
    }
}
