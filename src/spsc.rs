use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::padded::CachePadded;

pub struct RingBuffer<T> {
    slots: Box<[UnsafeCell<MaybeUninit<T>>]>,
    mask: usize,
    head: CachePadded<AtomicUsize>,
    tail: CachePadded<AtomicUsize>,
}

// SAFETY: значения передаются между потоками целиком, поэтому достаточно T: Send;
// к каждой ячейке в любой момент обращается только один из двух концов очереди.
unsafe impl<T: Send> Sync for RingBuffer<T> {}

pub struct Producer<'a, T> {
    ring: &'a RingBuffer<T>,
    tail: usize,
    cached_head: usize,
}

pub struct Consumer<'a, T> {
    ring: &'a RingBuffer<T>,
    head: usize,
    cached_tail: usize,
}

impl<T> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1).next_power_of_two();
        let slots = (0..capacity)
            .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
            .collect();
        Self {
            slots,
            mask: capacity - 1,
            head: CachePadded::new(AtomicUsize::new(0)),
            tail: CachePadded::new(AtomicUsize::new(0)),
        }
    }

    pub fn capacity(&self) -> usize {
        self.mask + 1
    }

    pub fn split(&mut self) -> (Producer<'_, T>, Consumer<'_, T>) {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);
        let ring: &RingBuffer<T> = self;
        (
            Producer {
                ring,
                tail,
                cached_head: head,
            },
            Consumer {
                ring,
                head,
                cached_tail: tail,
            },
        )
    }
}

impl<T> Drop for RingBuffer<T> {
    fn drop(&mut self) {
        let mut head = *self.head.get_mut();
        let tail = *self.tail.get_mut();
        while head != tail {
            // SAFETY: ячейки в диапазоне [head, tail) инициализированы и не прочитаны,
            // а &mut self исключает любые другие обращения к очереди.
            unsafe {
                (*self.slots[head & self.mask].get()).assume_init_drop();
            }
            head = head.wrapping_add(1);
        }
    }
}

impl<T> Producer<'_, T> {
    pub fn try_push(&mut self, value: T) -> Result<(), T> {
        let ring = self.ring;
        if self.tail.wrapping_sub(self.cached_head) == ring.capacity() {
            self.cached_head = ring.head.load(Ordering::Acquire);
            if self.tail.wrapping_sub(self.cached_head) == ring.capacity() {
                return Err(value);
            }
        }
        // SAFETY: Producer единственный пишет в ячейки, и эта ячейка свободна: читатель
        // освободил её, опубликовав head через release, а мы прочитали его через acquire.
        unsafe {
            (*ring.slots[self.tail & ring.mask].get()).write(value);
        }
        self.tail = self.tail.wrapping_add(1);
        ring.tail.store(self.tail, Ordering::Release);
        Ok(())
    }

    pub fn capacity(&self) -> usize {
        self.ring.capacity()
    }
}

impl<T> Consumer<'_, T> {
    pub fn try_pop(&mut self) -> Option<T> {
        let ring = self.ring;
        if self.head == self.cached_tail {
            self.cached_tail = ring.tail.load(Ordering::Acquire);
            if self.head == self.cached_tail {
                return None;
            }
        }
        // SAFETY: Consumer единственный читает из ячеек, и эта ячейка заполнена: писатель
        // опубликовал tail через release, а мы прочитали его через acquire.
        let value = unsafe { (*ring.slots[self.head & ring.mask].get()).assume_init_read() };
        self.head = self.head.wrapping_add(1);
        ring.head.store(self.head, Ordering::Release);
        Some(value)
    }

    pub fn capacity(&self) -> usize {
        self.ring.capacity()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Report {
    pub popped: u64,
    pub sum: u64,
    pub ordered: bool,
    pub capacity: usize,
    pub elapsed: Duration,
}

pub fn run(capacity: usize, items: u64) -> Report {
    let mut ring = RingBuffer::<u64>::new(capacity);
    let actual = ring.capacity();
    let (mut producer, mut consumer) = ring.split();
    let started = Instant::now();
    let (popped, sum, ordered) = thread::scope(|scope| {
        scope.spawn(move || {
            for value in 1..=items {
                let mut pending = value;
                while let Err(back) = producer.try_push(pending) {
                    pending = back;
                    std::hint::spin_loop();
                }
            }
        });
        let reader = scope.spawn(move || {
            let mut popped = 0u64;
            let mut sum = 0u64;
            let mut ordered = true;
            let mut expected = 1u64;
            while popped < items {
                match consumer.try_pop() {
                    Some(value) => {
                        ordered &= value == expected;
                        expected += 1;
                        sum += value;
                        popped += 1;
                    }
                    None => std::hint::spin_loop(),
                }
            }
            (popped, sum, ordered)
        });
        reader.join().expect("поток читателя завершился с паникой")
    });
    Report {
        popped,
        sum,
        ordered,
        capacity: actual,
        elapsed: started.elapsed(),
    }
}
