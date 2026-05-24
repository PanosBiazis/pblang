// Enhanced Concurrency Module for pblang
// Implements safe concurrency primitives similar to Go, Rust, and Kotlin

use pblang_ir::Value;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    Arc, Condvar, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// SharedValue
// ---------------------------------------------------------------------------

/// Thread-safe reference counting for shared values
pub struct SharedValue<T> {
    inner: Arc<RwLock<T>>,
    ref_count: Arc<AtomicUsize>,
}

impl<T> SharedValue<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(RwLock::new(value)),
            ref_count: Arc::new(AtomicUsize::new(1)),
        }
    }

    pub fn read(&self) -> Result<T, String>
    where
        T: Clone,
    {
        self.inner
            .read()
            .map(|guard| (*guard).clone())
            .map_err(|e| format!("Read lock failed: {}", e))
    }

    pub fn write(&self, value: T) -> Result<(), String> {
        self.inner
            .write()
            .map(|mut guard| *guard = value)
            .map_err(|e| format!("Write lock failed: {}", e))
    }

    pub fn clone_ref(&self) -> Self {
        self.ref_count.fetch_add(1, Ordering::SeqCst);
        Self {
            inner: Arc::clone(&self.inner),
            ref_count: Arc::clone(&self.ref_count),
        }
    }
}

impl<T> Clone for SharedValue<T> {
    fn clone(&self) -> Self {
        self.clone_ref()
    }
}

impl<T> Drop for SharedValue<T> {
    fn drop(&mut self) {
        self.ref_count.fetch_sub(1, Ordering::SeqCst);
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for SharedValue<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.inner.read() {
            Ok(guard) => f
                .debug_struct("SharedValue")
                .field("value", &*guard)
                .field("ref_count", &self.ref_count.load(Ordering::SeqCst))
                .finish(),
            Err(_) => f
                .debug_struct("SharedValue")
                .field("value", &"<locked>")
                .field("ref_count", &self.ref_count.load(Ordering::SeqCst))
                .finish(),
        }
    }
}

// ---------------------------------------------------------------------------
// SafeMutex
// ---------------------------------------------------------------------------

/// Mutex with timeout support.
///
/// Deadlock detection has been removed in favor of Rust's built-in mutex
/// poisoning. Use `try_lock` or `lock_timeout` for non-blocking acquisition.
pub struct SafeMutex<T> {
    inner: Arc<Mutex<T>>,
    timeout_duration: Duration,
}

impl<T> SafeMutex<T> {
    pub fn new(value: T, timeout_duration: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(value)),
            timeout_duration,
        }
    }

    /// Acquire the mutex lock, blocking until available.
    /// Relies on Rust's std Mutex poisoning for error reporting.
    pub fn lock(&self) -> Result<MutexGuard<'_, T>, String> {
        self.inner
            .lock()
            .map_err(|e| format!("Mutex lock failed: {}", e))
    }

    pub fn try_lock(&self) -> Result<MutexGuard<'_, T>, String> {
        self.inner
            .try_lock()
            .map_err(|e| format!("Mutex try_lock failed: {}", e))
    }

    /// Attempt to acquire the mutex lock with a timeout, using `try_lock`
    /// in a spin-wait loop with a short sleep between attempts.
    pub fn lock_timeout(&self, timeout: Duration) -> Result<MutexGuard<'_, T>, String> {
        let deadline = Instant::now() + timeout;
        let sleep_interval = self.timeout_duration.min(Duration::from_millis(1));
        loop {
            match self.inner.try_lock() {
                Ok(guard) => return Ok(guard),
                Err(std::sync::TryLockError::WouldBlock) => {
                    if Instant::now() >= deadline {
                        return Err("Mutex lock timeout".to_string());
                    }
                    thread::sleep(sleep_interval);
                }
                Err(std::sync::TryLockError::Poisoned(e)) => {
                    return Err(format!("Mutex lock poisoned: {}", e));
                }
            }
        }
    }
}

impl<T> std::fmt::Debug for SafeMutex<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SafeMutex")
            .field("timeout_duration", &self.timeout_duration)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// SafeRwLock
// ---------------------------------------------------------------------------

/// Read-Write Lock with timeout support.
///
/// Reader/writer tracking fields have been removed since they were not
/// maintained accurately. The underlying std::sync::RwLock handles
/// proper reader/writer management internally.
pub struct SafeRwLock<T> {
    inner: Arc<RwLock<T>>,
    #[allow(dead_code)]
    lock_timeout: Duration,
}

impl<T> SafeRwLock<T> {
    pub fn new(value: T, lock_timeout: Duration) -> Self {
        Self {
            inner: Arc::new(RwLock::new(value)),
            lock_timeout,
        }
    }

    pub fn read(&self) -> Result<RwLockReadGuard<'_, T>, String> {
        self.inner
            .read()
            .map_err(|e| format!("RwLock read failed: {}", e))
    }

    pub fn write(&self) -> Result<RwLockWriteGuard<'_, T>, String> {
        self.inner
            .write()
            .map_err(|e| format!("RwLock write failed: {}", e))
    }

    pub fn read_timeout(&self, timeout: Duration) -> Result<RwLockReadGuard<'_, T>, String> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.inner.try_read() {
                Ok(guard) => return Ok(guard),
                Err(std::sync::TryLockError::WouldBlock) => {
                    if Instant::now() >= deadline {
                        return Err("RwLock read timeout".to_string());
                    }
                    thread::sleep(Duration::from_micros(100));
                }
                Err(std::sync::TryLockError::Poisoned(e)) => {
                    return Err(format!("RwLock read poisoned: {}", e));
                }
            }
        }
    }

    pub fn write_timeout(&self, timeout: Duration) -> Result<RwLockWriteGuard<'_, T>, String> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.inner.try_write() {
                Ok(guard) => return Ok(guard),
                Err(std::sync::TryLockError::WouldBlock) => {
                    if Instant::now() >= deadline {
                        return Err("RwLock write timeout".to_string());
                    }
                    thread::sleep(Duration::from_micros(100));
                }
                Err(std::sync::TryLockError::Poisoned(e)) => {
                    return Err(format!("RwLock write poisoned: {}", e));
                }
            }
        }
    }
}

impl<T> std::fmt::Debug for SafeRwLock<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SafeRwLock")
            .field("lock_timeout", &self.lock_timeout)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// SafeChannel
// ---------------------------------------------------------------------------

/// Thread-safe channel with backpressure and flow control.
///
/// The senders/receivers tracking fields have been removed since they
/// were never updated. If sender/receiver tracking is needed in the
/// future, it should be integrated with RAII guards returned from
/// registration methods.
pub struct SafeChannel<T> {
    queue: Arc<Mutex<VecDeque<T>>>,
    capacity: Option<usize>,
    closed: Arc<AtomicBool>,
    send_cv: Arc<Condvar>,
    recv_cv: Arc<Condvar>,
}

impl<T> SafeChannel<T> {
    pub fn new(capacity: Option<usize>) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            capacity,
            closed: Arc::new(AtomicBool::new(false)),
            send_cv: Arc::new(Condvar::new()),
            recv_cv: Arc::new(Condvar::new()),
        }
    }

    pub fn send(&self, value: T, timeout: Option<Duration>) -> Result<(), String> {
        if self.closed.load(Ordering::SeqCst) {
            return Err("Channel is closed".to_string());
        }

        let mut queue = self
            .queue
            .lock()
            .map_err(|e| format!("Channel send lock failed: {}", e))?;

        // Check capacity limits
        if let Some(cap) = self.capacity {
            let deadline = timeout.map(|t| Instant::now() + t);
            while queue.len() >= cap && !self.closed.load(Ordering::SeqCst) {
                if let Some(dl) = deadline {
                    if Instant::now() >= dl {
                        return Err("Channel send timeout".to_string());
                    }
                    let remaining = dl.saturating_duration_since(Instant::now());
                    let (new_queue, timeout_result) =
                        self.send_cv
                            .wait_timeout(queue, remaining)
                            .map_err(|e| format!("Channel send wait failed: {}", e))?;
                    queue = new_queue;
                    if timeout_result.timed_out() {
                        return Err("Channel send timeout".to_string());
                    }
                } else {
                    queue = self
                        .send_cv
                        .wait(queue)
                        .map_err(|e| format!("Channel send wait failed: {}", e))?;
                }
            }
        }

        if self.closed.load(Ordering::SeqCst) {
            return Err("Channel is closed".to_string());
        }

        queue.push_back(value);
        drop(queue);
        self.recv_cv.notify_one();
        Ok(())
    }

    pub fn recv(&self, timeout: Option<Duration>) -> Result<T, String> {
        let mut queue = self
            .queue
            .lock()
            .map_err(|e| format!("Channel recv lock failed: {}", e))?;

        let deadline = timeout.map(|t| Instant::now() + t);
        while queue.is_empty() && !self.closed.load(Ordering::SeqCst) {
            if let Some(dl) = deadline {
                if Instant::now() >= dl {
                    return Err("Channel recv timeout".to_string());
                }
                let remaining = dl.saturating_duration_since(Instant::now());
                let (new_queue, timeout_result) = self
                    .recv_cv
                    .wait_timeout(queue, remaining)
                    .map_err(|e| format!("Channel recv wait failed: {}", e))?;
                queue = new_queue;
                if timeout_result.timed_out() {
                    return Err("Channel recv timeout".to_string());
                }
            } else {
                queue = self
                    .recv_cv
                    .wait(queue)
                    .map_err(|e| format!("Channel recv wait failed: {}", e))?;
            }
        }

        if let Some(value) = queue.pop_front() {
            drop(queue);
            self.send_cv.notify_one();
            Ok(value)
        } else if self.closed.load(Ordering::SeqCst) {
            Err("Channel is closed and empty".to_string())
        } else {
            Err("Channel is empty".to_string())
        }
    }

    pub fn try_send(&self, value: T) -> Result<(), String> {
        if self.closed.load(Ordering::SeqCst) {
            return Err("Channel is closed".to_string());
        }

        let mut queue = self
            .queue
            .lock()
            .map_err(|e| format!("Channel try_send lock failed: {}", e))?;

        if let Some(cap) = self.capacity {
            if queue.len() >= cap {
                return Err("Channel is full".to_string());
            }
        }

        queue.push_back(value);
        drop(queue);
        self.recv_cv.notify_one();
        Ok(())
    }

    pub fn try_recv(&self) -> Result<T, String> {
        let mut queue = self
            .queue
            .lock()
            .map_err(|e| format!("Channel try_recv lock failed: {}", e))?;

        if let Some(value) = queue.pop_front() {
            drop(queue);
            self.send_cv.notify_one();
            Ok(value)
        } else if self.closed.load(Ordering::SeqCst) {
            Err("Channel is closed and empty".to_string())
        } else {
            Err("Channel is empty".to_string())
        }
    }

    pub fn close(&self) -> Result<(), String> {
        self.closed.store(true, Ordering::SeqCst);
        self.send_cv.notify_all();
        self.recv_cv.notify_all();
        Ok(())
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    pub fn len(&self) -> Result<usize, String> {
        let queue = self
            .queue
            .lock()
            .map_err(|e| format!("Channel len lock failed: {}", e))?;
        Ok(queue.len())
    }

    pub fn is_empty(&self) -> Result<bool, String> {
        self.len().map(|len| len == 0)
    }
}

impl<T> std::fmt::Debug for SafeChannel<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SafeChannel")
            .field("capacity", &self.capacity)
            .field("closed", &self.closed.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// ThreadPool
// ---------------------------------------------------------------------------

/// Thread pool with work stealing and graceful shutdown.
///
/// Workers wait on a condition variable associated with the work queue
/// mutex, which avoids both busy-waiting and the lost-wakeup problem.
type Job = Box<dyn FnOnce() + Send>;
type JobQueue = Arc<Mutex<VecDeque<Job>>>;

pub struct ThreadPool {
    workers: Vec<Worker>,
    work_queue: JobQueue,
    idle_workers: Arc<Condvar>,
    shutdown: Arc<AtomicBool>,
    #[allow(dead_code)]
    thread_count: usize,
}

struct Worker {
    id: usize,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for Worker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Worker")
            .field("id", &self.id)
            .field("thread", &self.thread.as_ref().map(|_| "JoinHandle"))
            .finish()
    }
}

impl ThreadPool {
    pub fn new(thread_count: usize) -> Result<Self, String> {
        let work_queue = Arc::new(Mutex::new(VecDeque::new()));
        let idle_workers = Arc::new(Condvar::new());
        let shutdown = Arc::new(AtomicBool::new(false));

        let mut workers = Vec::with_capacity(thread_count);
        for id in 0..thread_count {
            let work_queue = Arc::clone(&work_queue);
            let idle_workers = Arc::clone(&idle_workers);
            let shutdown = Arc::clone(&shutdown);

            let thread = thread::spawn(move || {
                Self::worker_loop(id, work_queue, idle_workers, shutdown);
            });

            workers.push(Worker {
                id,
                thread: Some(thread),
            });
        }

        Ok(Self {
            workers,
            work_queue,
            idle_workers,
            shutdown,
            thread_count,
        })
    }

    pub fn execute<F>(&self, work: F) -> Result<(), String>
    where
        F: FnOnce() + Send + 'static,
    {
        if self.shutdown.load(Ordering::SeqCst) {
            return Err("ThreadPool is shutdown".to_string());
        }

        let mut queue = self
            .work_queue
            .lock()
            .map_err(|e| format!("ThreadPool execute lock failed: {}", e))?;
        queue.push_back(Box::new(work));
        drop(queue);
        self.idle_workers.notify_one();
        Ok(())
    }

    fn worker_loop(
        _id: usize,
        work_queue: JobQueue,
        idle_workers: Arc<Condvar>,
        shutdown: Arc<AtomicBool>,
    ) {
        fn lock_recover<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
            m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
        }

        loop {
            // Acquire the queue lock and try to get work.
            // If no work is available, wait on the condition variable
            // (which is associated with the queue mutex). This avoids
            // the lost-wakeup problem because we hold the queue lock
            // while checking for both work and shutdown.
            let work = {
                let mut queue = lock_recover(&work_queue);
                loop {
                    if let Some(work) = queue.pop_front() {
                        break Some(work);
                    }
                    if shutdown.load(Ordering::SeqCst) {
                        break None;
                    }
                    // Release the lock and wait. When we wake up,
                    // we re-check for work and shutdown.
                    match idle_workers.wait_timeout(queue, Duration::from_secs(1)) {
                        Ok((next_queue, _)) => queue = next_queue,
                        Err(poisoned) => {
                            let (next_queue, _) = poisoned.into_inner();
                            queue = next_queue;
                        }
                    }
                }
            };

            match work {
                Some(w) => w(),
                None => break, // shutdown signaled, no remaining work
            }
        }
    }

    pub fn shutdown(&self) -> Result<(), String> {
        self.shutdown.store(true, Ordering::SeqCst);
        self.idle_workers.notify_all();

        // We can't join here since we only have &self not &mut self
        // The joins happen on Drop or via join_all
        Ok(())
    }

    pub fn join_all(&mut self) -> Result<(), String> {
        self.shutdown.store(true, Ordering::SeqCst);
        self.idle_workers.notify_all();

        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                thread
                    .join()
                    .map_err(|_| "Failed to join worker thread".to_string())?;
            }
        }
        Ok(())
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.idle_workers.notify_all();
        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

impl std::fmt::Debug for ThreadPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreadPool")
            .field("thread_count", &self.thread_count)
            .field("shutdown", &self.shutdown.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Atomic
// ---------------------------------------------------------------------------

/// Marker for types whose bit-pattern arithmetic on `AtomicU64` is meaningful.
/// `fetch_add` is gated on this trait so callers can't accidentally use it on
/// types like `f64` or raw pointers — the underlying op is integer addition on
/// the bit pattern, which is nonsensical for floats and unsound for pointers.
///
/// # Safety
/// Implementors must be 8 bytes and treat their bit pattern as an unsigned
/// integer for the purposes of addition.
pub unsafe trait AtomicAddable: Copy + 'static {}

unsafe impl AtomicAddable for u64 {}
unsafe impl AtomicAddable for i64 {}
unsafe impl AtomicAddable for usize {} // 64-bit targets; assertion below catches 32-bit.

/// Atomic operations wrapper with type safety.
/// Only works with types that are the same size as u64 (8 bytes).
///
/// `load`, `store`, and `compare_exchange` work on any `Copy + 'static` type
/// of the right size. `fetch_add` is restricted to types implementing
/// `AtomicAddable` so a hypothetical `Atomic<f64>::fetch_add` (which would
/// do integer addition on float bits — silently wrong) won't compile.
///
/// The 8-byte size requirement is enforced by a `const { assert! }` block
/// in `new`, so `Atomic::<u32>::new(0)` fails at compile-time per
/// monomorphisation — no runtime panic surface.
pub struct Atomic<T: Copy + 'static> {
    value: AtomicU64,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Copy + 'static> Atomic<T> {
    pub fn new(value: T) -> Self {
        // Compile-time size check: monomorphisations with the wrong T fail to
        // compile rather than panic at runtime. Requires Rust 1.79+
        // (inline const blocks); the workspace pins toolchain >= 1.79.
        const {
            assert!(
                std::mem::size_of::<T>() == 8,
                "Atomic<T> only supports types that are exactly 8 bytes (size_of::<u64>())"
            );
        }
        Self {
            value: AtomicU64::new(unsafe { std::mem::transmute_copy::<T, u64>(&value) }),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn load(&self) -> T {
        let raw = self.value.load(Ordering::SeqCst);
        unsafe { std::mem::transmute_copy::<u64, T>(&raw) }
    }

    pub fn store(&self, value: T) {
        let raw: u64 = unsafe { std::mem::transmute_copy::<T, u64>(&value) };
        self.value.store(raw, Ordering::SeqCst);
    }

    pub fn compare_exchange(&self, current: T, new: T) -> Result<T, T> {
        let cur_raw: u64 = unsafe { std::mem::transmute_copy::<T, u64>(&current) };
        let new_raw: u64 = unsafe { std::mem::transmute_copy::<T, u64>(&new) };
        match self
            .value
            .compare_exchange(cur_raw, new_raw, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) => Ok(current),
            Err(actual_raw) => {
                let actual: T = unsafe { std::mem::transmute_copy::<u64, T>(&actual_raw) };
                Err(actual)
            }
        }
    }
}

impl<T: AtomicAddable> Atomic<T> {
    /// Integer-only atomic add. Restricted via the `AtomicInteger` trait so
    /// non-integer 8-byte types (e.g. `f64`) can't reach a nonsensical
    /// bit-pattern fetch_add.
    pub fn fetch_add(&self, value: T) -> T {
        let raw: u64 = unsafe { std::mem::transmute_copy::<T, u64>(&value) };
        let prev_raw = self.value.fetch_add(raw, Ordering::SeqCst);
        unsafe { std::mem::transmute_copy::<u64, T>(&prev_raw) }
    }
}

impl<T: Copy + 'static + std::fmt::Debug> std::fmt::Debug for Atomic<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Atomic")
            .field("value", &self.load())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Barrier
// ---------------------------------------------------------------------------

/// Internal state protected by the Barrier's mutex.
struct BarrierState {
    waiting: usize,
    generation: usize,
}

/// Barrier for synchronizing multiple threads.
///
/// `wait()` blocks until all `n` threads have arrived, then releases them.
/// Returns `Ok(true)` for the "leader" thread (the last to arrive),
/// and `Ok(false)` for all other threads, similar to Rust's
/// `std::sync::Barrier`.
pub struct Barrier {
    n: usize,
    state: Mutex<BarrierState>,
    cv: Condvar,
}

impl Barrier {
    pub fn new(count: usize) -> Self {
        Self {
            n: count,
            state: Mutex::new(BarrierState {
                waiting: 0,
                generation: 0,
            }),
            cv: Condvar::new(),
        }
    }

    /// Wait for all threads to arrive at the barrier.
    ///
    /// Returns `Ok(true)` if this thread is the "leader" (the last to arrive),
    /// `Ok(false)` otherwise. Returns an error if the wait times out.
    pub fn wait(&self) -> Result<bool, String> {
        let mut guard = self
            .state
            .lock()
            .map_err(|e| format!("Barrier wait lock failed: {}", e))?;

        let current_gen = guard.generation;
        guard.waiting += 1;

        if guard.waiting == self.n {
            // Last thread to arrive — this is the "leader"
            guard.waiting = 0;
            guard.generation += 1;
            self.cv.notify_all();
            Ok(true)
        } else {
            // Wait for the leader thread to signal
            while guard.generation == current_gen {
                let result = self
                    .cv
                    .wait_timeout(guard, Duration::from_secs(30))
                    .map_err(|e| format!("Barrier wait failed: {}", e))?;
                guard = result.0;
                if result.1.timed_out() {
                    return Err("Barrier wait timeout".to_string());
                }
            }
            Ok(false)
        }
    }
}

impl std::fmt::Debug for Barrier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state.lock().ok();
        f.debug_struct("Barrier")
            .field("n", &self.n)
            .field("waiting", &state.as_ref().map(|s| s.waiting).unwrap_or(0))
            .field(
                "generation",
                &state.as_ref().map(|s| s.generation).unwrap_or(0),
            )
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Once
// ---------------------------------------------------------------------------

/// Once flag for one-time initialization
pub struct Once {
    initialized: AtomicBool,
    value: Rc<RefCell<Option<Value>>>,
}

impl Once {
    pub fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            value: Rc::new(RefCell::new(None)),
        }
    }

    pub fn call_once<F>(&self, f: F) -> Result<(), String>
    where
        F: FnOnce() -> Value,
    {
        if self
            .initialized
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            let value = f();
            self.value.replace(Some(value));
            Ok(())
        } else {
            Err("Once already called".to_string())
        }
    }

    pub fn get(&self) -> Option<Value> {
        self.value.borrow().clone()
    }
}

impl std::fmt::Debug for Once {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Once")
            .field("initialized", &self.initialized.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

impl Default for Once {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Timer
// ---------------------------------------------------------------------------

/// Timer for delayed operations.
///
/// Uses `thread::sleep` for waiting instead of a condvar that no one
/// signals. Supports cancellation via the `cancel()` method.
pub struct Timer {
    deadline: Arc<AtomicU64>,
    completed: Arc<AtomicBool>,
}

impl Timer {
    fn now_millis() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    pub fn new(duration: Duration) -> Self {
        let deadline = Self::now_millis().saturating_add(duration.as_millis() as u64);

        Self {
            deadline: Arc::new(AtomicU64::new(deadline)),
            completed: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Wait until the timer expires or is cancelled.
    ///
    /// Uses `thread::sleep` in small intervals so that cancellation
    /// via `cancel()` is responsive.
    pub fn wait(&self) -> Result<(), String> {
        loop {
            if self.completed.load(Ordering::SeqCst) {
                return Ok(());
            }
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("Timer wait time error: {}", e))?;
            let now_ms = now.as_millis() as u64;
            let deadline_ms = self.deadline.load(Ordering::SeqCst);
            if now_ms >= deadline_ms {
                self.completed.store(true, Ordering::SeqCst);
                return Ok(());
            }
            // Sleep in small intervals to allow cancellation
            let remaining = Duration::from_millis(deadline_ms - now_ms);
            let sleep_time = remaining.min(Duration::from_millis(100));
            thread::sleep(sleep_time);
        }
    }

    pub fn is_expired(&self) -> bool {
        let now = Self::now_millis();
        now >= self.deadline.load(Ordering::SeqCst)
    }

    /// Cancel the timer, causing `wait()` to return immediately.
    pub fn cancel(&self) {
        self.completed.store(true, Ordering::SeqCst);
    }

    /// Returns the deadline as milliseconds since the Unix epoch.
    pub fn deadline_ms(&self) -> u64 {
        self.deadline.load(Ordering::SeqCst)
    }

    /// Returns the remaining duration until the timer expires.
    /// Returns a zero duration if the timer has already expired.
    pub fn remaining(&self) -> Duration {
        let now = Self::now_millis();
        let deadline_ms = self.deadline.load(Ordering::SeqCst);
        Duration::from_millis(deadline_ms.saturating_sub(now))
    }
}

impl std::fmt::Debug for Timer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Timer")
            .field("deadline_ms", &self.deadline.load(Ordering::SeqCst))
            .field("completed", &self.completed.load(Ordering::SeqCst))
            .finish()
    }
}

// ---------------------------------------------------------------------------
// ConcurrencyStats
// ---------------------------------------------------------------------------

/// Concurrency statistics for monitoring
pub struct ConcurrencyStats {
    pub active_threads: usize,
    pub pending_tasks: usize,
    pub active_channels: usize,
    pub active_mutexes: usize,
    pub total_operations: u64,
}

impl ConcurrencyStats {
    pub fn new() -> Self {
        Self {
            active_threads: thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
            pending_tasks: 0,
            active_channels: 0,
            active_mutexes: 0,
            total_operations: 0,
        }
    }
}

impl Default for ConcurrencyStats {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ConcurrencyStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConcurrencyStats")
            .field("active_threads", &self.active_threads)
            .field("pending_tasks", &self.pending_tasks)
            .field("active_channels", &self.active_channels)
            .field("active_mutexes", &self.active_mutexes)
            .field("total_operations", &self.total_operations)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// VmThreadSupervisor — tracked VM threads with timeout monitoring
// ---------------------------------------------------------------------------

/// Tracks spawned VM threads with heartbeat monitoring and automatic cleanup.
/// Inspired by Erlang's supervisor trees and Rust's JoinHandle.
pub struct VmThreadSupervisor {
    handles: Arc<Mutex<HashMap<usize, SupervisedThread>>>,
    next_id: AtomicUsize,
    stats: Arc<Mutex<SupervisorStats>>,
}

struct SupervisedThread {
    deadline: Instant,
}

#[derive(Debug, Clone, Default)]
pub struct SupervisorStats {
    pub spawned: u64,
    pub completed: u64,
    pub timed_out: u64,
    pub killed: u64,
    pub active: usize,
}

impl Default for VmThreadSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl VmThreadSupervisor {
    pub fn new() -> Self {
        Self {
            handles: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicUsize::new(1),
            stats: Arc::new(Mutex::new(SupervisorStats::default())),
        }
    }

    /// Spawn a new VM thread with a timeout. The given closure receives a
    /// sender to report completion and should spawn the VM accordingly.
    ///
    /// Returns a unique thread ID that can be used with `check()`.
    pub fn supervise<F>(&self, timeout_ms: u64, spawn_fn: F) -> usize
    where
        F: FnOnce() -> JoinHandle<Result<SpawnWireValue, String>> + Send + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let handle = spawn_fn();
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);

        self.stats.lock().unwrap_or_else(|e| e.into_inner()).spawned += 1;

        let handles = self.handles.clone();
        let stats = self.stats.clone();
        let deadline_for_monitor = deadline;

        // Monitor: poll until join completes or deadline passes (cannot force-kill OS threads).
        thread::spawn(move || {
            while !handle.is_finished() {
                if Instant::now() >= deadline_for_monitor {
                    let mut h = handles.lock().unwrap_or_else(|e| e.into_inner());
                    let mut s = stats.lock().unwrap_or_else(|e| e.into_inner());
                    s.timed_out += 1;
                    h.remove(&id);
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
            let result = handle.join();
            let mut h = handles.lock().unwrap_or_else(|e| e.into_inner());
            let mut s = stats.lock().unwrap_or_else(|e| e.into_inner());
            match &result {
                Ok(Ok(_)) => s.completed += 1,
                Ok(Err(_)) => s.completed += 1,
                Err(_) => s.timed_out += 1,
            }
            h.remove(&id);
        });

        self.handles.lock().unwrap_or_else(|e| e.into_inner()).insert(
            id,
            SupervisedThread { deadline },
        );
        id
    }

    /// Check if a supervised thread has timed out, and if so, remove it.
    /// Returns `true` if the thread was found and still alive.
    pub fn check(&self, id: usize) -> bool {
        let mut handles = self.handles.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = handles.get(&id) {
            if Instant::now() >= t.deadline {
                let mut s = self.stats.lock().unwrap_or_else(|e| e.into_inner());
                s.timed_out += 1;
                handles.remove(&id);
                return false;
            }
            return true;
        }
        false
    }

    /// Check all supervised threads, removing any that have timed out or finished.
    pub fn check_all(&self) {
        let mut handles = self.handles.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let timed_out: Vec<usize> = handles
            .iter()
            .filter(|(_, t)| now >= t.deadline)
            .map(|(id, _)| *id)
            .collect();
        {
            let mut s = self.stats.lock().unwrap_or_else(|e| e.into_inner());
            s.timed_out += timed_out.len() as u64;
        }
        for id in timed_out {
            handles.remove(&id);
        }
    }

    /// Kill all supervised threads (marks them as timed out for cleanup).
    pub fn kill_all(&self) {
        let mut handles = self.handles.lock().unwrap_or_else(|e| e.into_inner());
        let count = handles.len();
        handles.clear();
        let mut s = self.stats.lock().unwrap_or_else(|e| e.into_inner());
        s.killed += count as u64;
    }

    /// Return a snapshot of supervision statistics.
    pub fn stats(&self) -> SupervisorStats {
        let s = self.stats.lock().unwrap_or_else(|e| e.into_inner());
        let h = self.handles.lock().unwrap_or_else(|e| e.into_inner());
        SupervisorStats {
            spawned: s.spawned,
            completed: s.completed,
            timed_out: s.timed_out,
            killed: s.killed,
            active: h.len(),
        }
    }

    /// Number of currently active supervised threads.
    pub fn active_count(&self) -> usize {
        self.handles.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Total threads ever spawned under this supervisor.
    pub fn total_spawned(&self) -> u64 {
        self.stats.lock().unwrap_or_else(|e| e.into_inner()).spawned
    }
}

impl std::fmt::Debug for VmThreadSupervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.stats();
        f.debug_struct("VmThreadSupervisor")
            .field("active", &s.active)
            .field("spawned", &s.spawned)
            .field("completed", &s.completed)
            .field("timed_out", &s.timed_out)
            .field("killed", &s.killed)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// AtomicInteger — atomic integer wrapper for pblang Value
// ---------------------------------------------------------------------------

/// Atomic integer operations on pblang values, backed by AtomicI64.
/// Provides CAS, increment, decrement, and exchange.
pub struct AtomicInteger {
    inner: AtomicI64,
}

impl AtomicInteger {
    pub fn new(value: i64) -> Self {
        Self {
            inner: AtomicI64::new(value),
        }
    }

    /// Load the current value.
    pub fn load(&self) -> i64 {
        self.inner.load(Ordering::SeqCst)
    }

    /// Store a new value.
    pub fn store(&self, value: i64) {
        self.inner.store(value, Ordering::SeqCst);
    }

    /// Compare-and-swap: if current == expected, set to new and return Ok(old).
    /// Otherwise return Err(actual).
    pub fn compare_exchange(&self, expected: i64, new: i64) -> Result<i64, i64> {
        self.inner
            .compare_exchange(expected, new, Ordering::SeqCst, Ordering::SeqCst)
    }

    /// Atomic fetch-and-add. Returns the previous value.
    pub fn fetch_add(&self, delta: i64) -> i64 {
        self.inner.fetch_add(delta, Ordering::SeqCst)
    }

    /// Atomic fetch-and-sub. Returns the previous value.
    pub fn fetch_sub(&self, delta: i64) -> i64 {
        self.inner.fetch_sub(delta, Ordering::SeqCst)
    }

    /// Atomic increment by 1. Returns the new value.
    pub fn increment(&self) -> i64 {
        self.inner.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Atomic decrement by 1. Returns the new value.
    pub fn decrement(&self) -> i64 {
        self.inner.fetch_sub(1, Ordering::SeqCst) - 1
    }

    /// Atomic exchange (swap). Returns the previous value.
    pub fn swap(&self, value: i64) -> i64 {
        self.inner.swap(value, Ordering::SeqCst)
    }

    /// Create from a pblang Value::Number (truncated to i64).
    pub fn from_value(v: &Value) -> Self {
        let n = match v {
            Value::Number(n) => *n as i64,
            Value::Bool(b) => {
                if *b {
                    1
                } else {
                    0
                }
            }
            _ => 0,
        };
        Self::new(n)
    }

    /// Return the current value as a pblang Value::Number.
    pub fn to_value(&self) -> Value {
        Value::Number(self.load() as f64)
    }
}

impl std::fmt::Debug for AtomicInteger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AtomicInteger")
            .field("value", &self.load())
            .finish()
    }
}

impl Clone for AtomicInteger {
    fn clone(&self) -> Self {
        Self {
            inner: AtomicI64::new(self.load()),
        }
    }
}

// ---------------------------------------------------------------------------
// VM handle registry (per-VM isolation; shared with spawned child VMs)
// ---------------------------------------------------------------------------

use pblang_ir::chunk::{OpCode, Span};
use std::collections::HashMap;
use std::sync::atomic::AtomicI64;
use std::sync::mpsc;

/// Spawn/join result channel: Ok = JSON return value, Err = error message.
pub type SpawnJoinReceiver = mpsc::Receiver<Result<SpawnWireValue, String>>;

/// Send-safe value representation used by spawn/join between VM threads.
#[derive(Clone, Debug)]
pub enum SpawnWireValue {
    Nil,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<SpawnWireValue>),
    Map(HashMap<String, SpawnWireValue>),
    Function(Box<SpawnWireFunction>),
    Closure(Box<SpawnWireClosure>),
}

#[derive(Clone, Debug)]
pub struct SpawnWireFunction {
    pub name: Option<String>,
    pub arity: usize,
    pub min_arity: usize,
    pub defaults: Vec<SpawnWireValue>,
    pub is_vararg: bool,
    pub instructions: Vec<OpCode>,
    pub spans: Vec<Span>,
    pub constants: Vec<SpawnWireValue>,
    pub param_types: Option<Vec<pblang_ir::ParamType>>,
}

#[derive(Clone, Debug)]
pub struct SpawnWireClosure {
    pub function: SpawnWireFunction,
    pub upvalues: Vec<SpawnWireValue>,
}

/// Buffered channel backing store for VM builtins (values are JSON strings).
#[derive(Clone)]
pub struct VmChannelBuf {
    pub buf: Arc<Mutex<std::collections::VecDeque<String>>>,
    pub closed: Arc<AtomicBool>,
    /// 0 = unbounded; >0 = bounded capacity.
    pub capacity: usize,
    pub space_cv: Arc<(Mutex<()>, Condvar)>,
    pub item_cv: Arc<(Mutex<()>, Condvar)>,
}

/// Mutex handle used by VM sync builtins.
pub struct VmSyncMutex {
    pub locked: Mutex<bool>,
    pub cv: Condvar,
}

/// RwLock handle used by VM sync builtins.
pub struct VmSyncRwLock {
    /// readers < 0 means write-locked (-1); readers >= 0 = reader count
    pub state: Mutex<i64>,
    pub cv: Condvar,
}

/// Barrier handle used by VM sync builtins.
pub struct VmSyncBarrier {
    pub n: usize,
    pub arrived: Mutex<usize>,
    pub cv: Condvar,
}

/// Once handle used by VM sync builtins.
pub struct VmSyncOnce {
    pub done: AtomicBool,
    pub running: Mutex<bool>,
    pub cv: Condvar,
}

/// Atomic i64 handle used by VM sync builtins.
pub struct VmSyncAtomic {
    pub val: AtomicI64,
}

/// Per-VM tables for spawn/join, channels, and sync primitives.
///
/// Parent and child VMs created via `spawn` share one registry (`Arc` clone).
/// Separate top-level `VM` instances do not share handles.
pub struct VmHandleRegistry {
    pub spawn: Mutex<HashMap<usize, SpawnJoinReceiver>>,
    pub spawn_next: AtomicUsize,
    pub channels: Mutex<HashMap<usize, VmChannelBuf>>,
    pub channel_next: AtomicUsize,
    pub mutexes: Mutex<HashMap<usize, Arc<VmSyncMutex>>>,
    pub mutex_next: AtomicUsize,
    pub rwlocks: Mutex<HashMap<usize, Arc<VmSyncRwLock>>>,
    pub rwlock_next: AtomicUsize,
    pub barriers: Mutex<HashMap<usize, Arc<VmSyncBarrier>>>,
    pub barrier_next: AtomicUsize,
    pub once_flags: Mutex<HashMap<usize, Arc<VmSyncOnce>>>,
    pub once_next: AtomicUsize,
    pub atomics: Mutex<HashMap<usize, Arc<VmSyncAtomic>>>,
    pub atomic_next: AtomicUsize,
}

impl VmHandleRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            spawn: Mutex::new(HashMap::new()),
            spawn_next: AtomicUsize::new(1),
            channels: Mutex::new(HashMap::new()),
            channel_next: AtomicUsize::new(1),
            mutexes: Mutex::new(HashMap::new()),
            mutex_next: AtomicUsize::new(1),
            rwlocks: Mutex::new(HashMap::new()),
            rwlock_next: AtomicUsize::new(1),
            barriers: Mutex::new(HashMap::new()),
            barrier_next: AtomicUsize::new(1),
            once_flags: Mutex::new(HashMap::new()),
            once_next: AtomicUsize::new(1),
            atomics: Mutex::new(HashMap::new()),
            atomic_next: AtomicUsize::new(1),
        })
    }
}
