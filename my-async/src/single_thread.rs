use super::reactor;

use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll},
};

use crossbeam::channel::{self, Receiver, Sender, TryRecvError};
use futures_lite::future::{Boxed, FutureExt};
use futures_task::{waker_ref, ArcWake};
use once_cell::sync::Lazy;
use parking_lot::Mutex;

static SPAWNER: Lazy<Mutex<Option<Spawner>>> = Lazy::new(|| Mutex::new(None));

struct Task {
    future: Mutex<Option<Boxed<()>>>,
    tx: Sender<Message>,
}

enum Message {
    Run(Arc<Task>),
    Close,
}

pub struct Executor {
    rx: Receiver<Message>,
}

struct Spawner {
    tx: Sender<Message>,
}

impl Executor {
    pub fn new() -> Self {
        let (tx, rx) = channel::unbounded();
        let spawner = Spawner { tx };
        SPAWNER.lock().replace(spawner);
        Self { rx }
    }

    fn run(&self) {
        let mut reactor = reactor::Reactor::default();
        reactor.setup_registry();
        loop {
            match self.rx.try_recv() {
                Ok(msg) => match msg {
                    // run task
                    Message::Run(task) => task.run(),
                    // received disconnect message, cleanup and exit.
                    Message::Close => break,
                },
                Err(TryRecvError::Empty) => {
                    // mio wait for io harvest
                    reactor.wait(None).unwrap();
                }
                // no one is connected, bye.
                Err(TryRecvError::Disconnected) => break,
            }
        }
    }
    pub fn block_on<F>(&self, future: F)
    where
        F: Future<Output = ()> + 'static + Send,
    {
        spawn(future);
        self.run()
    }
}

impl Drop for Executor {
    fn drop(&mut self) {
        if let Some(spawner) = SPAWNER.lock().as_ref() {
            spawner
                .tx
                .send(Message::Close)
                .expect("Message queue is full.");
        }
    }
}

impl Task {
    pub fn run(self: &Arc<Self>) {
        let mut future_slot = self.future.lock();
        // run *ONCE*
        if let Some(mut future) = future_slot.take() {
            let waker = waker_ref(self);
            let cx = &mut Context::from_waker(&waker);
            match future.as_mut().poll(cx) {
                Poll::Ready(r) => return r,
                Poll::Pending => {
                    *future_slot = Some(future);
                }
            };
        }
    }
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let clone = Arc::clone(arc_self);
        arc_self
            .tx
            .send(Message::Run(clone))
            .expect("Too many message queued!");
    }
}

impl Spawner {
    fn spawn<F>(&self, fut: F)
    where
        F: Future<Output = ()> + 'static + Send,
    {
        let future = fut.boxed();
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            tx: self.tx.clone(),
        });
        // let _scope = super::enter::enter().unwrap();
        self.tx
            .send(Message::Run(task))
            .expect("too many task queued");
    }
}

pub fn spawn<F>(fut: F)
where
    F: Future<Output = ()> + 'static + Send,
{
    if let Some(spawner) = SPAWNER.lock().as_ref() {
        spawner.spawn(fut);
    }
}
