use super::{Task, TaskId};
use alloc::{collections::BTreeMap, sync::Arc, task::Wake};
use core::task::{Context, Poll, Waker};
use crossbeam_queue::ArrayQueue;

pub struct Executor {
    tasks: BTreeMap<TaskId, Task>,
    task_queue: Arc<ArrayQueue<TaskId>>,
    waker_cache: BTreeMap<TaskId, Waker>,
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            tasks: BTreeMap::new(),
            task_queue: Arc::new(ArrayQueue::new(100)),
            waker_cache: BTreeMap::new(),
        }
    }

    /// Spawns a new task.
    ///
    /// Because we are making a mutable loan from the executor, we can no longer execute `spawn` after the `run`
    /// method starts executing, plus `run` implements an infinite loop with a divergent return.
    /// One solution is to create a custom `Spawner` type that shares a queue with the `Executor`, a queue shared with it,
    /// or its own queue that is synchronized by the `Executor`.
    ///
    /// Remember that Rust doesn't allow having two mutable borrows at the same time, except for reborrowing.
    pub fn spawn(&mut self, task: Task) {
        let task_id = task.id;

        if self.tasks.insert(task.id, task).is_some() {
            panic!("task with same ID already in tasks");
        }

        self.task_queue.push(task_id).expect("queue full");
    }

    fn run_ready_tasks(&mut self) {
        // Destructuring is necessary because in the closure below we attempt to perform a full borrow of
        // self in order to obtain the waker_cache.
        let Self {
            tasks,
            task_queue,
            waker_cache,
        } = self;

        while let Some(task_id) = task_queue.pop() {
            let task = match tasks.get_mut(&task_id) {
                Some(task) => task,
                // Task no longer exists.
                None => continue,
            };

            let waker = waker_cache
                .entry(task_id)
                .or_insert_with(|| TaskWaker::new(task_id, task_queue.clone()));

            let mut context = Context::from_waker(waker);

            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    // If the task is complete, remove it and its curly waker. There's no reason to keep them,
                    // since the task is finished.
                    tasks.remove(&task_id);
                    waker_cache.remove(&task_id);
                }

                Poll::Pending => {}
            }
        }
    }

    /// The executor spins.
    ///
    /// Because the keyboard task, for example, prevents the tasks map from being empty, a loop with a
    /// divergent return value should model such tasks.
    pub fn run(&mut self) -> ! {
        loop {
            self.run_ready_tasks();
            self.sleep_if_idle();
        }
    }

    /// It puts the CPU into sleep mode when there are no tasks in the task queue, preventing the CPU from becoming busy.
    fn sleep_if_idle(&self) {
        use x86_64::instructions::interrupts::{self, enable_and_hlt};

        interrupts::disable(); // Prevent race conditions
        // Between run_ready_tasks and sleep_if_idle, an interruption may occur and the queue may not become empty, hence the new check.
        if self.task_queue.is_empty() {
            // We disabled interrupts earlier because if an interrupt happens here, we'll lose the wakeup.
            // After verifying that there are indeed no tasks in the queue, we re-enable interrupts and activate
            // the hlt instruction to enter sleep mode. This is all done atomically.
            enable_and_hlt();
        } else {
            // This means that after run_ready_tasks a new task was added by an interrupt, so we re-enable interrupts
            // and re-enter the loop.
            interrupts::enable();
        }
    }
}

/// The waker's job is to push the waken task ID to the task_queue.
/// Next, the `Executor` polls for the new task.
struct TaskWaker {
    task_id: TaskId,
    // Ownership of task_queue is shared between wakers and executors through the Arc wrapper type,
    // which is based on reference counting.
    task_queue: Arc<ArrayQueue<TaskId>>,
}

impl TaskWaker {
    fn new(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Waker {
        // The Waker type supports conversions using the From trait when the type in question implements the Wake trait.
        // This is because we are wrapping a type that implements the Wake trait, where this trait uses the Arc smart pointer.
        Waker::from(Arc::new(TaskWaker {
            task_id,
            task_queue,
        }))
    }

    fn wake_task(&self) {
        self.task_queue.push(self.task_id).expect("task_queue full");
    }
}

// We need to instantiate a Waker from the TaskWaker, where the simplest way is to
// implement the Wake trait, which is based on Arc.
// It's based on Arc because wakers are commonly shared between executors and asynchronous tasks.
// The Waker type supports conversions using the From trait when the type in question implements the Wake trait.
impl Wake for TaskWaker {
    // Since this captures ownership, it increases the number of references in Arc.
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }

    // Implementing this method is optional because not all data types support waking by reference.
    // However, implementing it provides performance benefits because it eliminates the need to modify
    // the reference count, for example.
    fn wake_by_ref(self: &Arc<Self>) {
        // Since our type only requires one &self reference, this is easy to resolve.
        self.wake_task();
    }
}
