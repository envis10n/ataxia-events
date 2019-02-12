use std::thread;
use crossbeam::channel::{unbounded, Sender, SendError};
use futures::future::{lazy, ok as fok};

/// Boxed function that can be sent between threads.
pub type Closure = Box<Fn() + Send>;

/// Enum defining the type of an event.
pub enum EventType {
    None,
    End,
    Call
}

/// A task to be sent to the event loop.
pub struct EventTask {
    event_type: EventType,
    payload: Option<Closure>
}

impl EventTask {
    /// Create a task that will cause the event loop to stop.
    pub fn end() -> EventTask {
        EventTask { event_type: EventType::End, payload: None }
    }
    /// Create a task that will run in the event loop.
    pub fn call(func: Closure) -> EventTask {
        EventTask { event_type: EventType::Call, payload: Some(func) }
    }
}

/// An asynchronous event loop that handles tasks.
pub struct EventLoop {
    sender: Sender<EventTask>,
    running: bool
}

impl EventLoop {
    /// Create and start a new EventLoop.
    pub fn new() -> EventLoop {
        let (tx, rx) = unbounded::<EventTask>();
        let eventloop = lazy(move || {
            while let Ok(task) = rx.recv() {
                match task.event_type {
                    EventType::None => (),
                    EventType::End => break,
                    EventType::Call => {
                        if let Some(call) = task.payload {
                            tokio::spawn(lazy(move || {
                                call();
                                fok::<(), ()>(())
                            }));
                        }
                    }
                }
            }
            drop(rx);
            fok::<(), ()>(())
        });
        thread::spawn(move || {
            tokio::run(eventloop);
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
    pub fn call<F: Fn() + Send + 'static>(&self, func: F) -> Result<(), SendError<EventTask>> {
        if self.running {
            self.sender.send(EventTask::call(Box::new(func)))
        } else {
            Ok(())
        }
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
            let mut eventloop = EventLoop::new();
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
