use std::panic::{self, AssertUnwindSafe};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

pub type ThreadResult<T> = thread::Result<T>;

pub struct ThreadPool {
    sender: mpsc::Sender<Message>,
    receiver: Arc<Mutex<mpsc::Receiver<Message>>>,
    workers: Vec<Worker>,
}

impl ThreadPool {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
            workers: Vec::new(),
        }
    }

    pub fn ensure_workers(&mut self, target: usize) {
        while self.workers.len() < target {
            self.spawn_worker();
        }
    }

    pub fn execute<F, R>(&self, job: F) -> mpsc::Receiver<ThreadResult<R>>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let (result_tx, result_rx) = mpsc::channel();
        let message = Message::Job(Box::new(move || {
            let result = panic::catch_unwind(AssertUnwindSafe(job));
            let _ = result_tx.send(result);
        }));
        self.sender
            .send(message)
            .expect("Failed to send thread pool job");
        result_rx
    }

    fn spawn_worker(&mut self) {
        let receiver = Arc::clone(&self.receiver);
        let handle = thread::spawn(move || loop {
            let message = receiver.lock().expect("Thread pool lock poisoned").recv();
            match message {
                Ok(Message::Job(job)) => job(),
                Ok(Message::Terminate) | Err(_) => break,
            }
        });
        self.workers.push(Worker { handle: Some(handle) });
    }
}

impl Default for ThreadPool {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        for _ in &self.workers {
            let _ = self.sender.send(Message::Terminate);
        }

        for worker in &mut self.workers {
            if let Some(handle) = worker.handle.take() {
                let _ = handle.join();
            }
        }
    }
}

struct Worker {
    handle: Option<thread::JoinHandle<()>>,
}

enum Message {
    Job(Box<dyn FnOnce() + Send + 'static>),
    Terminate,
}
