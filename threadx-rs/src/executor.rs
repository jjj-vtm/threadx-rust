use core::{
    ffi::CStr, future::Future, mem, pin::{pin, Pin}, task::Context
};

use defmt::println;
use static_cell::StaticCell;

use crate::{
    mutex::{self, Mutex},
    queue::{Queue, QueueReceiver, QueueSender},
};
extern crate alloc;
/// Task executor that receives tasks off of a channel and runs them.

pub struct Executor {
    ready_queue: QueueReceiver<alloc::sync::Arc<Task>>,
}

/// `Spawner` spawns new futures onto the task channel.
//#[derive(Clone)]
pub struct Spawner {
    task_sender: QueueSender<alloc::sync::Arc<Task>>,
}
pub type BoxFuture<'a, T> = Pin<alloc::boxed::Box<dyn Future<Output = T> + Send + 'a>>;

/// A future that can reschedule itself to be polled by an `Executor`.
pub struct Task {
    /// In-progress future that should be pushed to completion.
    ///
    /// The `Mutex` is not necessary for correctness, since we only have
    /// one thread executing tasks at once. However, Rust isn't smart
    /// enough to know that `future` is only mutated from one thread,
    /// so we need to use the `Mutex` to prove thread-safety. A production
    /// executor would not need this, and could use `UnsafeCell` instead.
    future: Mutex<Option<BoxFuture<'static, ()>>>,

    /// Handle to place the task itself back onto the task queue.
    task_sender: QueueSender<alloc::sync::Arc<Task>>,
}
impl alloc::task::Wake for Task {
    fn wake(self: alloc::sync::Arc<Self>) {
        // Clone self and resend to queue to wake it up.
        self.task_sender
            .send(self.clone(), crate::WaitOption::NoWait)
            .unwrap();
    }
}

static QUEUE_MEM: StaticCell<[u8; 128]> = StaticCell::new();
static TASK_QUEUE: StaticCell<Queue<alloc::sync::Arc<Task>>> = StaticCell::new();
const MAX_QUEUED_TASKS: usize = 32;

pub fn new_executor_and_spawner() -> (Executor, Spawner) {
    // Maximum number of tasks to allow queueing in the channel at once.
    // This is just to make `sync_channel` happy, and wouldn't be present in
    // a real executor.
    let queue = TASK_QUEUE.init(Queue::new());
    let name = CStr::from_bytes_with_nul(b"TaskQueue\0").unwrap();
    let mem = QUEUE_MEM.init([0u8; MAX_QUEUED_TASKS * 4]);
    let (task_sender, ready_queue) = queue.initialize(name, mem).unwrap();
    (Executor { ready_queue }, Spawner { task_sender })
}

impl Spawner {
    pub fn spawn(&self, future: impl Future<Output = ()> + Send + 'static ) {
        println!("Pinning the future in spawn");
        let future = alloc::boxed::Box::pin(future);
        
        println!("Pin created.");
        let mut mutex: Mutex<Option<Pin<alloc::boxed::Box<dyn Future<Output = ()> + Send + '_>>>> = Mutex::new(Some(future));
        let _ = mutex.initialize(CStr::from_bytes_until_nul(b"task1\0").unwrap(), false).unwrap();
        let task = alloc::sync::Arc::new(Task {
            future: mutex, 
            task_sender: self.task_sender.clone(),
        });
        // TODO: In order to be safe the heap allocations have to be leaked and they should be dropped on the receiving side.
                 self.task_sender
            .send(task, crate::WaitOption::WaitForever)
            .expect("too many tasks queued");
    }
}

impl Executor {
    pub fn run(&self) {
        while let Ok(task) = self.ready_queue.receive(crate::WaitOption::WaitForever) {
            // Take the future, and if it has not yet completed (is still Some),
            // poll it in an attempt to complete it.
            println!("Running a future");
            let mut future_slot = task.future.lock(crate::WaitOption::WaitForever).unwrap();
            if let Some(mut future) = future_slot.take() {
                // Create a `LocalWaker` from the task itself
                println!("Pin at: {}", &*future as *const _);
                let waker = task.clone().into();
                let context = &mut Context::from_waker(&waker);
                // `BoxFuture<T>` is a type alias for
                // `Pin<Box<dyn Future<Output = T> + Send + 'static>>`.
                // We can get a `Pin<&mut dyn Future + Send + 'static>`
                // from it by calling the `Pin::as_mut` method.
                if future.as_mut().poll(context).is_pending() {
                    // We're not done processing the future, so put it
                    // back in its task to be run again in the future.
                    *future_slot = Some(future);
                }
            }
        }
    }
}
