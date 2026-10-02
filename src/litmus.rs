use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::thread;

use crate::counter::{MemOrder, Order, Relaxed, SeqCst};
use crate::padded::CachePadded;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcomes {
    pub iterations: u64,
    pub both_zero: u64,
    pub r1_only: u64,
    pub r2_only: u64,
    pub both_one: u64,
}

struct Shared {
    x: CachePadded<AtomicU32>,
    y: CachePadded<AtomicU32>,
    start: [CachePadded<AtomicU64>; 2],
    done: [CachePadded<AtomicU64>; 2],
    result: [CachePadded<AtomicU32>; 2],
}

impl Shared {
    fn new() -> Self {
        let round = || CachePadded::new(AtomicU64::new(0));
        let value = || CachePadded::new(AtomicU32::new(0));
        Self {
            x: value(),
            y: value(),
            start: [round(), round()],
            done: [round(), round()],
            result: [value(), value()],
        }
    }
}

fn next_random(state: &mut u64) -> u64 {
    let mut s = *state;
    s ^= s << 13;
    s ^= s >> 7;
    s ^= s << 17;
    *state = s;
    s
}

fn delay(state: &mut u64) {
    while next_random(state) % 10 != 0 {
        std::hint::spin_loop();
    }
}

fn worker<O: Order>(shared: &Shared, id: usize, rounds: u64) {
    let (mine, theirs) = if id == 0 {
        (&shared.x, &shared.y)
    } else {
        (&shared.y, &shared.x)
    };
    let mut rng = 0x9E37_79B9_7F4A_7C15u64 ^ ((id as u64 + 1) << 32);
    for round in 1..=rounds {
        while shared.start[id].load(Ordering::Acquire) != round {
            std::hint::spin_loop();
        }
        delay(&mut rng);
        mine.store(1, O::RMW);
        let seen = theirs.load(O::LOAD);
        shared.result[id].store(seen, Ordering::Relaxed);
        shared.done[id].store(round, Ordering::Release);
    }
}

fn run_typed<O: Order>(n: u64) -> Outcomes {
    let shared = Shared::new();
    let mut outcomes = Outcomes {
        iterations: n,
        ..Outcomes::default()
    };
    thread::scope(|scope| {
        for id in 0..2 {
            let shared = &shared;
            scope.spawn(move || worker::<O>(shared, id, n));
        }
        for round in 1..=n {
            shared.x.store(0, Ordering::Relaxed);
            shared.y.store(0, Ordering::Relaxed);
            shared.start[0].store(round, Ordering::Release);
            shared.start[1].store(round, Ordering::Release);
            for id in 0..2 {
                while shared.done[id].load(Ordering::Acquire) != round {
                    thread::yield_now();
                }
            }
            let r1 = shared.result[0].load(Ordering::Relaxed);
            let r2 = shared.result[1].load(Ordering::Relaxed);
            match (r1, r2) {
                (0, 0) => outcomes.both_zero += 1,
                (1, 0) => outcomes.r1_only += 1,
                (0, 1) => outcomes.r2_only += 1,
                _ => outcomes.both_one += 1,
            }
        }
    });
    outcomes
}

pub fn run(n: u64, order: MemOrder) -> Outcomes {
    match order {
        MemOrder::Relaxed => run_typed::<Relaxed>(n),
        MemOrder::SeqCst => run_typed::<SeqCst>(n),
    }
}
