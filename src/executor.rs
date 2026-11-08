// Copyright (c) 2024 Rune Skovbo Johansen
// SPDX-License-Identifier: MPL-2.0

use std::any::Any;
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak};

pub type Task = Box<dyn FnOnce(StopToken) + Send + 'static>;
pub type Panic = Box<dyn Any + Send + 'static>;
pub type ErrorSink = Arc<dyn Fn(Panic) + Send + Sync + 'static>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownPolicy {
    Drain,
    CancelPending,
}

type StopCallback = Arc<dyn Fn() + Send + Sync>;
#[derive(Default)]
struct StopState {
    stopped: AtomicBool,
    next: AtomicU64,
    callbacks: Mutex<BTreeMap<u64, StopCallback>>,
}

#[derive(Clone, Default)]
pub struct StopToken(Arc<StopState>);

impl StopToken {
    pub fn stop_requested(&self) -> bool {
        self.0.stopped.load(Ordering::Acquire)
    }

    pub fn register(&self, callback: impl Fn() + Send + Sync + 'static) -> StopRegistration {
        let callback: StopCallback = Arc::new(callback);
        let mut callbacks = self.0.callbacks.lock().unwrap();
        if self.stop_requested() {
            drop(callbacks);
            let _ = catch_unwind(AssertUnwindSafe(|| callback()));
            return StopRegistration {
                state: Weak::new(),
                id: 0,
            };
        }
        let id = self.0.next.fetch_add(1, Ordering::Relaxed);
        callbacks.insert(id, callback);
        StopRegistration {
            state: Arc::downgrade(&self.0),
            id,
        }
    }

    fn request_stop(&self) {
        if self.0.stopped.swap(true, Ordering::AcqRel) {
            return;
        }
        let ids: Vec<_> = self.0.callbacks.lock().unwrap().keys().copied().collect();
        for id in ids {
            let callback = self.0.callbacks.lock().unwrap().remove(&id);
            if let Some(callback) = callback {
                let _ = catch_unwind(AssertUnwindSafe(|| callback()));
            }
        }
    }
}

pub struct StopRegistration {
    state: Weak<StopState>,
    id: u64,
}
impl Drop for StopRegistration {
    fn drop(&mut self) {
        if let Some(state) = self.state.upgrade() {
            let removed = state.callbacks.lock().unwrap().remove(&self.id);
            drop(removed);
        }
    }
}

struct Work {
    accepting: bool,
    active: usize,
    tasks: VecDeque<Task>,
}
struct Control {
    work: Mutex<Work>,
    changed: Condvar,
    stop: StopToken,
    sink: Option<ErrorSink>,
    reporting: Mutex<()>,
}
thread_local! { static CURRENT: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) }; }
struct ExecutionScope;
impl Drop for ExecutionScope {
    fn drop(&mut self) {
        CURRENT.with(|current| {
            current.borrow_mut().pop();
        });
    }
}

impl Control {
    fn new(sink: Option<ErrorSink>) -> Arc<Self> {
        Arc::new(Self {
            work: Mutex::new(Work {
                accepting: true,
                active: 0,
                tasks: VecDeque::new(),
            }),
            changed: Condvar::new(),
            stop: StopToken::default(),
            sink,
            reporting: Mutex::new(()),
        })
    }
    fn current(&self) -> bool {
        CURRENT.with(|current| current.borrow().contains(&(self as *const Self as usize)))
    }
    fn run(&self, task: Task) {
        CURRENT.with(|current| current.borrow_mut().push(self as *const Self as usize));
        let _scope = ExecutionScope;
        if let Err(error) = catch_unwind(AssertUnwindSafe(|| task(self.stop.clone())))
            && let Some(sink) = &self.sink
        {
            let _guard = self
                .reporting
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let _ = catch_unwind(AssertUnwindSafe(|| sink(error)));
        }
    }
    fn close(&self, policy: ShutdownPolicy) {
        let cancelled = {
            let mut work = self.work.lock().unwrap();
            work.accepting = false;
            if policy == ShutdownPolicy::CancelPending {
                std::mem::take(&mut work.tasks)
            } else {
                VecDeque::new()
            }
        };
        if policy == ShutdownPolicy::CancelPending {
            self.stop.request_stop();
        }
        self.changed.notify_all();
        drop(cancelled);
    }
}

pub trait Executor {
    fn execute(&self, task: Task) -> bool;
    fn request_stop(&self);
    fn shutdown(&self, policy: ShutdownPolicy);
}

pub struct InlineExecutor {
    control: Arc<Control>,
}
impl Default for InlineExecutor {
    fn default() -> Self {
        Self::new()
    }
}
impl InlineExecutor {
    pub fn new() -> Self {
        Self {
            control: Control::new(None),
        }
    }
    pub fn with_error_sink(sink: ErrorSink) -> Self {
        Self {
            control: Control::new(Some(sink)),
        }
    }
}
impl Executor for InlineExecutor {
    fn execute(&self, task: Task) -> bool {
        let control = self.control.clone();
        {
            let mut work = control.work.lock().unwrap();
            if !work.accepting {
                return false;
            }
            work.active += 1;
        }
        control.run(task);
        control.work.lock().unwrap().active -= 1;
        control.changed.notify_all();
        true
    }
    fn request_stop(&self) {
        self.control.close(ShutdownPolicy::CancelPending);
    }
    fn shutdown(&self, policy: ShutdownPolicy) {
        self.control.close(policy);
        if self.control.current() {
            return;
        }
        let mut work = self.control.work.lock().unwrap();
        while work.active > 0 {
            work = self.control.changed.wait(work).unwrap();
        }
    }
}
impl Drop for InlineExecutor {
    fn drop(&mut self) {
        self.shutdown(ShutdownPolicy::Drain);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct ManagedExecutor {
    control: Arc<Control>,
    workers: Mutex<Vec<std::thread::JoinHandle<()>>>,
    count: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl ManagedExecutor {
    pub fn new(workers: usize) -> std::io::Result<Self> {
        Self::with_optional_sink(workers, None)
    }
    pub fn with_error_sink(workers: usize, sink: ErrorSink) -> std::io::Result<Self> {
        Self::with_optional_sink(workers, Some(sink))
    }
    fn with_optional_sink(count: usize, sink: Option<ErrorSink>) -> std::io::Result<Self> {
        if count == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "worker count must be positive",
            ));
        }
        let control = Control::new(sink);
        let mut workers = Vec::with_capacity(count);
        for index in 0..count {
            let state = control.clone();
            match std::thread::Builder::new()
                .name(format!("layerprocgen-{index}"))
                .spawn(move || {
                    loop {
                        let task = {
                            let mut work = state.work.lock().unwrap();
                            while work.tasks.is_empty() && work.accepting {
                                work = state.changed.wait(work).unwrap();
                            }
                            if state.stop.stop_requested() {
                                break;
                            }
                            match work.tasks.pop_front() {
                                Some(task) => task,
                                None => break,
                            }
                        };
                        state.run(task);
                    }
                }) {
                Ok(worker) => workers.push(worker),
                Err(error) => {
                    control.close(ShutdownPolicy::CancelPending);
                    for worker in workers {
                        let _ = worker.join();
                    }
                    return Err(error);
                }
            }
        }
        Ok(Self {
            control,
            workers: Mutex::new(workers),
            count,
        })
    }
    pub fn worker_count(&self) -> usize {
        self.count
    }
    pub fn pending(&self) -> usize {
        self.control.work.lock().unwrap().tasks.len()
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl Executor for ManagedExecutor {
    fn execute(&self, task: Task) -> bool {
        {
            let mut work = self.control.work.lock().unwrap();
            if !work.accepting {
                return false;
            }
            work.tasks.push_back(task);
        }
        self.control.changed.notify_one();
        true
    }
    fn request_stop(&self) {
        self.control.close(ShutdownPolicy::CancelPending);
    }
    fn shutdown(&self, policy: ShutdownPolicy) {
        self.control.close(policy);
        if self.control.current() {
            return;
        }
        let mut workers = self.workers.lock().unwrap();
        for worker in workers.drain(..) {
            let _ = worker.join();
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl Drop for ManagedExecutor {
    fn drop(&mut self) {
        if self.control.current() {
            self.control.close(ShutdownPolicy::Drain);
            let workers = std::mem::take(&mut *self.workers.lock().unwrap());
            let _ = std::thread::Builder::new()
                .name("layerprocgen-reaper".into())
                .spawn(move || {
                    for worker in workers {
                        let _ = worker.join();
                    }
                });
        } else {
            self.shutdown(ShutdownPolicy::Drain);
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;
    const TIMEOUT: Duration = Duration::from_secs(3);

    #[test]
    fn inline_order_and_reentrant_cancellation() {
        let executor = Arc::new(InlineExecutor::new());
        let events = Arc::new(Mutex::new(Vec::new()));
        for value in 0..4 {
            let events = events.clone();
            assert!(executor.execute(Box::new(move |token| {
                assert!(!token.stop_requested());
                events.lock().unwrap().push(value);
            })));
        }
        assert_eq!(*events.lock().unwrap(), vec![0, 1, 2, 3]);
        let owner = executor.clone();
        assert!(executor.execute(Box::new(move |token| {
            let weak = Arc::downgrade(&owner);
            let _registration = token.register(move || {
                assert!(
                    !weak
                        .upgrade()
                        .unwrap()
                        .execute(Box::new(|_| panic!("accepted work after stop")))
                );
            });
            owner.shutdown(ShutdownPolicy::CancelPending);
            assert!(token.stop_requested());
        })));
    }
    #[test]
    fn managed_fifo_and_drain_preserve_every_task() {
        let executor = ManagedExecutor::new(1).unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        for value in 0..128 {
            let events = events.clone();
            assert!(executor.execute(Box::new(move |token| {
                assert!(!token.stop_requested());
                events.lock().unwrap().push(value);
            })));
        }
        executor.shutdown(ShutdownPolicy::Drain);
        assert_eq!(*events.lock().unwrap(), (0..128).collect::<Vec<_>>());
        assert!(!executor.execute(Box::new(|_| panic!("accepted after shutdown"))));
    }
    #[test]
    fn cancellation_discards_captures_outside_queue_lock() {
        let executor = Arc::new(ManagedExecutor::new(1).unwrap());
        let (started, waiting) = mpsc::channel();
        let (observed, received) = mpsc::channel();
        assert!(executor.execute(Box::new(move |token| {
            started.send(()).unwrap();
            while !token.stop_requested() {
                std::thread::yield_now();
            }
            observed.send(()).unwrap();
        })));
        waiting.recv_timeout(TIMEOUT).unwrap();
        struct Capture(Weak<ManagedExecutor>, mpsc::Sender<()>);
        impl Drop for Capture {
            fn drop(&mut self) {
                assert_eq!(self.0.upgrade().unwrap().pending(), 0);
                self.1.send(()).unwrap();
            }
        }
        let (dropped, drop_received) = mpsc::channel();
        let capture = Capture(Arc::downgrade(&executor), dropped);
        assert!(executor.execute(Box::new(move |_| {
            drop(capture);
            panic!("cancelled task ran");
        })));
        executor.request_stop();
        drop_received.recv_timeout(TIMEOUT).unwrap();
        received.recv_timeout(TIMEOUT).unwrap();
        executor.shutdown(ShutdownPolicy::Drain);
    }
    #[test]
    fn task_and_reporter_panics_do_not_kill_worker() {
        let calls = Arc::new(AtomicU64::new(0));
        let called = calls.clone();
        let executor = ManagedExecutor::with_error_sink(
            1,
            Arc::new(move |_| {
                called.fetch_add(1, Ordering::Relaxed);
                panic!("reporter failed");
            }),
        )
        .unwrap();
        assert!(executor.execute(Box::new(|_| panic!("task failed"))));
        let (completed, received) = mpsc::channel();
        assert!(executor.execute(Box::new(move |_| completed.send(()).unwrap())));
        executor.shutdown(ShutdownPolicy::Drain);
        received.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn concurrent_producers_do_not_lose_work() {
        let executor = Arc::new(ManagedExecutor::new(4).unwrap());
        let count = Arc::new(AtomicU64::new(0));
        let producers: Vec<_> = (0..8)
            .map(|_| {
                let executor = executor.clone();
                let count = count.clone();
                std::thread::spawn(move || {
                    for _ in 0..250 {
                        let count = count.clone();
                        assert!(executor.execute(Box::new(move |_| {
                            count.fetch_add(1, Ordering::Relaxed);
                        })));
                    }
                })
            })
            .collect();
        for producer in producers {
            producer.join().unwrap();
        }
        executor.shutdown(ShutdownPolicy::Drain);
        assert_eq!(count.load(Ordering::Relaxed), 2000);
    }
    #[test]
    fn last_facade_owner_can_drop_on_worker() {
        let executor = Arc::new(ManagedExecutor::new(2).unwrap());
        let weak = Arc::downgrade(&executor);
        let owner = executor.clone();
        let (release, ready) = mpsc::channel();
        let (done, finished) = mpsc::channel();
        assert!(executor.execute(Box::new(move |_| {
            ready.recv_timeout(TIMEOUT).unwrap();
            drop(owner);
            done.send(()).unwrap();
        })));
        drop(executor);
        release.send(()).unwrap();
        finished.recv_timeout(TIMEOUT).unwrap();
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn callback_shutdown_can_race_external_shutdown() {
        let executor = Arc::new(ManagedExecutor::new(1).unwrap());
        let owner = executor.clone();
        let (entered, running) = mpsc::channel();
        let (release, ready) = mpsc::channel();
        assert!(executor.execute(Box::new(move |_| {
            entered.send(()).unwrap();
            ready.recv_timeout(TIMEOUT).unwrap();
            owner.shutdown(ShutdownPolicy::CancelPending);
        })));
        running.recv_timeout(TIMEOUT).unwrap();
        let external = executor.clone();
        let joiner = std::thread::spawn(move || external.shutdown(ShutdownPolicy::Drain));
        release.send(()).unwrap();
        joiner.join().unwrap();
    }
}
