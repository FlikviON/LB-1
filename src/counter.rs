use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::padded::CachePadded;

pub trait Order {
    const RMW: Ordering;
    const LOAD: Ordering;
}

pub struct Relaxed;

pub struct SeqCst;

impl Order for Relaxed {
    const RMW: Ordering = Ordering::Relaxed;
    const LOAD: Ordering = Ordering::Relaxed;
}

impl Order for SeqCst {
    const RMW: Ordering = Ordering::SeqCst;
    const LOAD: Ordering = Ordering::SeqCst;
}

pub struct Counter<O: Order> {
    value: CachePadded<AtomicU64>,
    _order: PhantomData<fn() -> O>,
}

impl<O: Order> Counter<O> {
    pub const fn new() -> Self {
        Self {
            value: CachePadded::new(AtomicU64::new(0)),
            _order: PhantomData,
        }
    }

    pub fn increment(&self) {
        self.value.fetch_add(1, O::RMW);
    }

    pub fn increment_cas(&self) {
        let mut current = self.value.load(Ordering::Relaxed);
        while let Err(actual) =
            self.value
                .compare_exchange_weak(current, current + 1, O::RMW, Ordering::Relaxed)
        {
            current = actual;
        }
    }

    pub fn get(&self) -> u64 {
        self.value.load(O::LOAD)
    }
}

impl<O: Order> Default for Counter<O> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemOrder {
    Relaxed,
    SeqCst,
}

impl MemOrder {
    pub fn name(self) -> &'static str {
        match self {
            MemOrder::Relaxed => "relaxed",
            MemOrder::SeqCst => "seqcst",
        }
    }
}

impl FromStr for MemOrder {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "relaxed" => Ok(MemOrder::Relaxed),
            "seqcst" => Ok(MemOrder::SeqCst),
            other => Err(format!(
                "неизвестный порядок памяти {other:?}, ожидалось relaxed или seqcst"
            )),
        }
    }
}

fn run_typed<O: Order>(threads: usize, n: u64, cas: bool) -> (u64, Duration) {
    let counter = Counter::<O>::new();
    let go = AtomicBool::new(false);
    let started = thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                while !go.load(Ordering::Acquire) {
                    std::hint::spin_loop();
                }
                if cas {
                    for _ in 0..n {
                        counter.increment_cas();
                    }
                } else {
                    for _ in 0..n {
                        counter.increment();
                    }
                }
            });
        }
        let started = Instant::now();
        go.store(true, Ordering::Release);
        started
    });
    let elapsed = started.elapsed();
    (counter.get(), elapsed)
}

pub fn run_timed(threads: usize, n: u64, order: MemOrder, cas: bool) -> (u64, Duration) {
    match order {
        MemOrder::Relaxed => run_typed::<Relaxed>(threads, n, cas),
        MemOrder::SeqCst => run_typed::<SeqCst>(threads, n, cas),
    }
}

pub fn run(threads: usize, n: u64, order: MemOrder, cas: bool) -> u64 {
    run_timed(threads, n, order, cas).0
}
