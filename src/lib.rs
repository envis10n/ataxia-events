use crossbeam::channel::{unbounded, Sender, SendError};
use futures::future::{loop_fn, FutureResult, lazy, ok as fok, Loop};
use std::collections::BTreeMap;
use std::thread;
use std::sync::{Arc, Mutex};

/// Boxed function that can be sent between threads.
pub type Closure = Box<Fn() + Send + Sync + 'static>;

pub type EventHooks = Arc<Mutex<BTreeMap<String, Closure>>>;

/// Enum defining the type of an event.
pub enum EventType {
    None,
    End,
    AddHook,
    RemoveHook,
    Call
}

/// A task to be sent to the event loop.
pub struct EventTask {
    event_type: EventType,
    payload: Option<Closure>,
    options: Option<Vec<String>>
}

impl EventTask {
    /// Create a task that will cause the event loop to stop.
    pub fn end() -> EventTask {
        EventTask { event_type: EventType::End, payload: None, options: None }
    }
    /// Create a task that will run in the event loop.
    pub fn call(func: Closure) -> EventTask {
        EventTask { event_type: EventType::Call, payload: Some(func), options: None }
    }
}

/// An asynchronous event loop that handles tasks.
#[derive(Debug, Clone)]
pub struct EventLoop {
    sender: Sender<EventTask>,
    running: bool
}

impl EventLoop {
    /// Create and start a new EventLoop.
    pub fn new() -> EventLoop {
        let (tx, rx) = unbounded::<EventTask>();
        let hookser: Arc<Mutex<BTreeMap<String, Closure>>> = Arc::new(Mutex::new(BTreeMap::new()));
        thread::spawn(move || {
            let looper = loop_fn((), |t| {
                let hooks_b = Arc::clone(&hookser);
                tokio::spawn(lazy(move || {
                    let hooks = Arc::clone(&hooks_b);
                    let guard = hooks.lock().unwrap();
                    for (key, cell) in (*guard).iter() {
                        tokio::spawn(lazy(move || {
                            cell();
                            fok::<(),()>(())
                        }));
                    }
                    fok::<(),()>(())
                }));
                if let Ok(task) = rx.recv() {
                    match task.event_type {
                        EventType::None => {
                            Ok::<Loop<(),()>, ()>(Loop::Continue(()))
                        },
                        EventType::End => {
                            drop(rx);
                            Ok::<Loop<(),()>, ()>(Loop::Break(()))
                        },
                        EventType::Call => {
                            if let Some(call) = task.payload {
                                tokio::spawn(lazy(move || {
                                    call();
                                    fok::<(), ()>(())
                                }));
                            }
                            Ok::<Loop<(),()>, ()>(Loop::Continue(()))
                        },
                        EventType::AddHook => {
                            if let Some(call) = task.payload {
                                if let Some(options) = task.options {
                                    let hooks = hooks_b.clone();
                                    let muh = hooks.clone();
                                    let mut guard = muh.lock().unwrap();
                                    let name = options[0].clone();
                                    (*guard).insert(name, call);
                                }
                            }
                            Ok::<Loop<(),()>, ()>(Loop::Continue(()))
                        },
                        EventType::RemoveHook => {
                            if let Some(options) = task.options {
                                let hooks = hooks_b.clone();
                                let muh = hooks.clone();
                                let mut guard = muh.lock().unwrap();
                                let name = &options[0];
                                (*guard).remove(name);
                            }
                            Ok::<Loop<(),()>, ()>(Loop::Continue(()))
                        }
                    }
                } else {
                    Ok::<Loop<(),()>, ()>(Loop::Continue(()))
                }
            });
        });
        EventLoop { sender: tx, running: true }
    }
    /// Send a task that stops an EventLoop.
    pub fn stop(&mut self) -> Result<(), SendError<EventTask>> {
        if self.running {
            self.running = false;
            self.sender.send(EventTask::end())
        } else {
            Ok(())
        }
    }
    /// Send a task that will be run in the EventLoop.
    pub fn call<F: Fn() + Send + Sync + 'static>(&self, func: F) -> Result<(), SendError<EventTask>> {
        if self.running {
            self.sender.send(EventTask::call(Box::new(func)))
        } else {
            Ok(())
        }
    }
    pub fn event_hooks() -> EventHooks {
        Arc::new(Mutex::new(BTreeMap::new()))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn event_loop() {
        use super::*;
        use std::thread;
        use std::time::Duration;

        // Spawn a separate thread to run the loop in.
        thread::spawn(|| {
            // Create the loop.
            let mut event_hooks = EventLoop::event_hooks();
            let mut eventloop = EventLoop::new(&event_hooks);
            // Send calls to be run as tasks.
            eventloop.call(|| {
                println!("Hello world 1!");
            }).unwrap();
            eventloop.call(|| {
                println!("Hello world 2!");
            }).unwrap();
            eventloop.call(|| {
                println!("Hello world 3!");
            }).unwrap();
            println!("Waiting 5 seconds to close...");
            thread::sleep(Duration::new(5, 0));
            // Stop the loop
            eventloop.stop().unwrap();
        }).join().unwrap();
    }
}
