// Allows for the single initialization of static variables.
use conquer_once::spin::OnceCell;

// Allows for a fixed-size queue without locks.
use crossbeam_queue::ArrayQueue;
use futures_util::stream::StreamExt;
use futures_util::task::AtomicWaker;
use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};
use pc_keyboard::{DecodedKey, HandleControl, ScancodeSet1, layouts};

use crate::{println, task::keyboard};

// We use `OnceCell` because `ArrayQueue::new` performs heap allocation, which is not allowed with static variables.
// We don't use `lazy-static` because we need to ensure predictable queue initialization.
// Otherwise, it could be initialized in interrupt handlers, which can lead to heap allocation.
static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();

// Store the `Waker` using `AtomicWaker`. We cannot use a field in `ScancodeStream` because it needs to be visible from `add_scancode`.
// `poll_next` as a consumer stores the wake.
// `add_scancode` as a producer triggers the wake.
static WAKER: AtomicWaker = AtomicWaker::new();

pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if let Err(_) = queue.push(scancode) {
            println!("WARNING: scancode queue full; dropping keyboard input");
        } else {
            WAKER.wake();
        }
    } else {
        println!("WARNING: scancode queue uninitialized");
    }
}

pub struct ScancodeStream {
    /// Prevents the struct from being constructed outside the module.
    _private: (),
}

impl ScancodeStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE
            .try_init_once(|| ArrayQueue::new(100))
            .expect("ScancodeStream::new should only be called once");

        ScancodeStream { _private: () }
    }
}

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Option<u8>> {
        let queue = SCANCODE_QUEUE
            .try_get()
            .expect("scancode queue not initialized");

        // Prevent initialization of WAKER. This is a fast path.
        if let Some(scancode) = queue.pop() {
            return Poll::Ready(Some(scancode));
        }

        WAKER.register(&ctx.waker());

        match queue.pop() {
            Some(scancode) => {
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            None => Poll::Pending,
        }
    }
}

pub async fn print_keypresses() {
    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(
        ScancodeSet1::new(),
        layouts::Us104Key,
        HandleControl::Ignore,
    );

    // It repeatedly obtains a scancode from the stream.
    // `.next` is obtained by the `StreamExt` trait, which returns a future that resolves to the next element in the stream.
    while let Some(scancode) = scancodes.next().await {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => print!("{}", character),
                    DecodedKey::RawKey(key) => print!("{:?}", key),
                }
            }
        }
    }
}
