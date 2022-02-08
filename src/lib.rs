#![allow(unused)]

use std::{sync::mpsc, thread};

/// The substringer subsystem
struct Substringer {
    value: String,
}

impl Substringer {
    fn new(value: String) -> Self {
        Self { value }
    }

    fn append_get(&mut self, other: &str) -> String {
        self.value += other;
        self.value.clone()
    }

    fn substr(&self, begin: usize, end: usize) -> String {
        self.value.get(begin..end).map_or_else(|| String::new(), str::to_string)
    }

    fn size(&self) -> usize {
        self.value.len()
    }
}

/// The counter subsystem
struct Counter {
    value: u64,
}

impl Counter {
    fn new() -> Self {
        Self { value: 13 }
    }

    fn get(&self) -> u64 {
        self.value
    }

    fn add_and_get(&mut self, amount: u64) -> u64 {
        self.value += amount;
        self.value
    }
}

/// Internal action type sent in the channel. None = shutdown subsystem.
type Action<T> = Option<Box<dyn FnOnce(&mut T) + Send>>;

/// Subsystem wrapper
struct Subsystem<T> {
    /// Send the subsystem stuff to do.
    tx: mpsc::Sender<Action<T>>,
    /// Subsystem thread handle.
    thread: Option<thread::JoinHandle<()>>,
}

impl<T: Send + 'static> Subsystem<T> {
    /// Spawn a subsystem in a new thread
    fn new(mut subsystem: T) -> Self {
        let (tx, rx): (mpsc::Sender<Action<T>>, _) = mpsc::channel();
        let thread = Some(thread::spawn(move || {
            while let Some(func) = rx.recv().unwrap() {
                func(&mut subsystem);
            }
        }));
        Self { tx, thread }
    }

    /// Dispatch a function call to the subsystem
    fn call_mut<R: Send + 'static>(
        &self,
        func: impl FnOnce(&mut T) -> R + Send + 'static,
    ) -> oneshot::Receiver<R> {
        let (rtx, rrx) = oneshot::channel::<R>();
        self.tx.send(Some(Box::new(move |subsys| {
            let result = func(subsys);
            rtx.send(result).unwrap();
        })));
        rrx
    }

    /// Dispatch a function call to the subsystem (convenience method if subsystem is not mutated)
    fn call<R: Send + 'static>(
        &self,
        func: impl FnOnce(&T) -> R + Send + 'static
    ) -> oneshot::Receiver<R> {
        self.call_mut(|this| func(this))
    }
}

impl<T> Drop for Subsystem<T> {
    fn drop(&mut self) {
        // Signal the thread to stop
        self.tx.send(None).unwrap();
        self.thread.take().map(thread::JoinHandle::join);
    }
}

#[test]
fn test() {
    let substr = Subsystem::new(Substringer::new("abc".into()));
    let counter = Subsystem::new(Counter::new());

    let res0 = substr.call_mut(|this| this.append_get("xyz"));
    assert_eq!(res0.recv().unwrap(), "abcxyz");

    let res1 = substr.call(|this| this.substr(2, 5));
    assert_eq!(res1.recv().unwrap(), "cxy");

    let res2 = counter.call(Counter::get);
    assert_eq!(res2.recv().unwrap(), 13);

    let res3 = counter.call_mut(|this| this.add_and_get(3));
    assert_eq!(res3.recv().unwrap(), 16);
}
