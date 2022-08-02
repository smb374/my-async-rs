use std::{
    marker::PhantomData,
    mem::{self, MaybeUninit},
    ptr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use cache_padded::CachePadded;

#[derive(Debug, PartialEq, Eq)]
pub enum Steal<T> {
    Empty,
    Retry,
    Success(T),
}

impl<T> Steal<T> {
    pub fn success(self) -> Option<T> {
        match self {
            Steal::Success(data) => Some(data),
            _ => None,
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, &Steal::Success(_))
    }

    pub fn is_retry(&self) -> bool {
        matches!(self, &Steal::Retry)
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, &Steal::Empty)
    }
}
struct Buffer<T> {
    ptr: *mut MaybeUninit<T>,
    cap: usize,
}

impl<T> Buffer<T> {
    fn new(cap: usize) -> Self {
        let mut v = Vec::with_capacity(cap);
        let ptr = v.as_mut_ptr();
        mem::forget(v);

        Self { ptr, cap }
    }

    unsafe fn dealloc(&self) {
        drop(Vec::from_raw_parts(self.ptr, self.cap, self.cap))
    }

    unsafe fn at(&self, index: usize) -> *mut MaybeUninit<T> {
        self.ptr.add(index)
    }

    unsafe fn read(&self, index: usize) -> T {
        ptr::read_volatile(self.at(index)).assume_init()
    }

    unsafe fn write(&self, index: usize, item: T) {
        ptr::write_volatile(self.at(index), MaybeUninit::new(item))
    }
}

impl<T> Clone for Buffer<T> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr,
            cap: self.cap,
        }
    }
}

struct Inner<T> {
    head: CachePadded<AtomicUsize>,
    tail: CachePadded<AtomicUsize>,
    // capacity will always be power of two for fast wrapping
    capacity: CachePadded<usize>,
    buffer: CachePadded<Buffer<T>>,
}

unsafe impl<T> Send for Inner<T> {}
unsafe impl<T> Sync for Inner<T> {}

// marker for thread-local only.
// omits some checks on push/pop
pub struct Ringbuf<T> {
    inner: Arc<CachePadded<Inner<T>>>,
    _marker: PhantomData<*mut ()>,
}

pub struct Stealer<T> {
    inner: Arc<CachePadded<Inner<T>>>,
}

impl<T> Inner<T> {
    fn len(&self) -> usize {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);

        tail.wrapping_sub(head)
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> Drop for Inner<T> {
    fn drop(&mut self) {
        unsafe {
            self.buffer.dealloc();
        }
    }
}

impl<T> Ringbuf<T> {
    pub fn new(cap: usize) -> Self {
        let capacity = if cap < usize::MAX && !cap.is_power_of_two() {
            cap.next_power_of_two()
        } else {
            cap
        };
        let buffer = CachePadded::new(Buffer::new(capacity));
        let inner = Arc::new(CachePadded::new(Inner {
            head: CachePadded::new(AtomicUsize::new(0)),
            tail: CachePadded::new(AtomicUsize::new(0)),
            capacity: CachePadded::new(capacity),
            buffer,
        }));

        Self {
            inner,
            _marker: PhantomData,
        }
    }

    pub fn stealer(&self) -> Stealer<T> {
        Stealer {
            inner: self.inner.clone(),
        }
    }

    // Ok(()): push success
    // Err(item): buffer full
    pub fn push(&self, item: T) -> Result<(), T> {
        let head = self.inner.head.load(Ordering::Relaxed);
        let tail = self.inner.tail.load(Ordering::Acquire);
        let capacity = self.inner.capacity;
        if tail.wrapping_sub(head) < *capacity {
            let new_tail = tail.wrapping_add(1) & (*capacity - 1);
            self.inner.tail.store(new_tail, Ordering::Release);
            // SAFETY:
            // The slot write here is either a fresh slot or read by `read` before
            unsafe {
                self.inner.buffer.write(tail, item);
            }
            Ok(())
        } else {
            Err(item)
        }
    }

    pub fn pop(&self) -> Option<T> {
        let head = self.inner.head.load(Ordering::Relaxed);
        let tail = self.inner.tail.load(Ordering::Acquire);
        if head == tail {
            None
        } else {
            let new_tail = tail.wrapping_sub(1) & (*self.inner.capacity - 1);
            self.inner.tail.store(new_tail, Ordering::Release);
            // SAFETY:
            // The slot read here will later be written by `write` later
            let item = unsafe { self.inner.buffer.read(new_tail) };
            Some(item)
        }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }
}

impl<T> Stealer<T> {
    pub fn steal(&self) -> Steal<T> {
        let tail = self.inner.tail.load(Ordering::Relaxed);
        let head = self.inner.head.load(Ordering::Acquire);

        if head == tail {
            return Steal::Empty;
        }
        let new_head = head.wrapping_add(1) & (*self.inner.capacity - 1);
        let result =
            self.inner
                .head
                .compare_exchange(head, new_head, Ordering::Release, Ordering::Relaxed);
        match result {
            Ok(_) => {
                // SAFETY:
                // The slot read here will later be written by `write` later
                let item = unsafe { self.inner.buffer.read(head) };
                Steal::Success(item)
            }
            Err(_) => Steal::Retry,
        }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Ringbuf, Steal, Stealer};

    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        thread::{self, JoinHandle},
    };

    use crossbeam_utils::sync::WaitGroup;

    #[test]
    fn single_thread_pop() {
        let ringbuf: Ringbuf<usize> = Ringbuf::new(256);

        assert_eq!(ringbuf.len(), 0);
        for &e in [1, 3, 5, 7, 9].iter() {
            ringbuf.push(e).unwrap();
        }
        assert_eq!(ringbuf.len(), 5);
        for &e in [9, 7, 5, 3, 1].iter() {
            assert_eq!(ringbuf.pop(), Some(e));
        }
        assert_eq!(ringbuf.pop(), None);
    }
    #[test]
    fn single_thread_steal() {
        let ringbuf: Ringbuf<usize> = Ringbuf::new(256);
        let stealer: Stealer<usize> = ringbuf.stealer();

        assert_eq!(ringbuf.len(), 0);
        for &e in [1, 3, 5, 7, 9].iter() {
            ringbuf.push(e).unwrap();
        }
        assert_eq!(ringbuf.len(), 5);
        for &e in [1, 3, 5, 7, 9].iter() {
            assert_eq!(stealer.steal(), Steal::Success(e));
        }
        assert_eq!(stealer.steal(), Steal::Empty);
        assert_eq!(ringbuf.pop(), None);
    }

    static COUNT: AtomicUsize = AtomicUsize::new(0);
    static ACC: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn parallel_steal() {
        let start = WaitGroup::new();
        let end = WaitGroup::new();
        let ringbuf: Ringbuf<usize> = Ringbuf::new(1024);
        let stealer = ringbuf.stealer();

        for _ in 0..8 {
            work(ringbuf.stealer(), start.clone(), end.clone());
        }
        for e in 0..1000 {
            ringbuf.push(e).unwrap();
        }
        assert_eq!(ringbuf.len(), 1000);
        start.wait(); // sync starting point
        end.wait(); // sync ending point
        assert_eq!(ringbuf.len(), 0);
        assert_eq!(stealer.steal(), Steal::Empty);
        assert_eq!(COUNT.load(Ordering::Relaxed), 1000);
        assert_eq!(ACC.load(Ordering::Relaxed), 499500);
    }
    fn work(stealer: Stealer<usize>, start: WaitGroup, end: WaitGroup) -> JoinHandle<()> {
        thread::spawn(move || {
            let mut x: Vec<usize> = Vec::new();
            start.wait(); // sync starting point
            loop {
                match stealer.steal() {
                    Steal::Empty => break,
                    Steal::Retry => {}
                    Steal::Success(s) => {
                        ACC.fetch_add(s, Ordering::Relaxed);
                        x.push(s);
                    }
                }
            }
            // println!("x: size = {}, content = {:?}", x.len(), x);
            COUNT.fetch_add(x.len(), Ordering::Relaxed);
            println!("x: size = {}", x.len());
            end.wait(); // sync ending point
        })
    }
}
