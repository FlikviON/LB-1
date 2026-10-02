use pp_lab::counter::{self, MemOrder};
use pp_lab::litmus;
use pp_lab::spsc::{self, RingBuffer};

#[test]
fn счётчик_точен_при_разном_числе_потоков() {
    for threads in [1, 2, 8, 16, 33] {
        for order in [MemOrder::Relaxed, MemOrder::SeqCst] {
            for cas in [false, true] {
                let total = counter::run(threads, 5_000, order, cas);
                assert_eq!(total, threads as u64 * 5_000);
            }
        }
    }
}

#[test]
fn счётчик_без_потоков_равен_нулю() {
    assert_eq!(counter::run(0, 1_000, MemOrder::SeqCst, false), 0);
    assert_eq!(counter::run(0, 1_000, MemOrder::Relaxed, true), 0);
}

#[test]
fn порядок_памяти_разбирается_из_строки() {
    assert_eq!("relaxed".parse::<MemOrder>(), Ok(MemOrder::Relaxed));
    assert_eq!("seqcst".parse::<MemOrder>(), Ok(MemOrder::SeqCst));
    assert!("acquire".parse::<MemOrder>().is_err());
}

#[test]
fn litmus_при_seqcst_не_даёт_оба_нуля() {
    let o = litmus::run(20_000, MemOrder::SeqCst);
    assert_eq!(o.both_zero, 0);
    assert_eq!(o.iterations, 20_000);
    assert_eq!(o.both_zero + o.r1_only + o.r2_only + o.both_one, 20_000);
}

#[test]
fn litmus_при_relaxed_сохраняет_сумму_исходов() {
    let o = litmus::run(20_000, MemOrder::Relaxed);
    assert_eq!(o.both_zero + o.r1_only + o.r2_only + o.both_one, 20_000);
}

#[test]
fn litmus_без_раундов() {
    let o = litmus::run(0, MemOrder::SeqCst);
    assert_eq!(o.iterations, 0);
    assert_eq!(o.both_zero + o.r1_only + o.r2_only + o.both_one, 0);
}

#[test]
fn ёмкость_округляется_до_степени_двойки() {
    assert_eq!(RingBuffer::<u32>::new(0).capacity(), 1);
    assert_eq!(RingBuffer::<u32>::new(1).capacity(), 1);
    assert_eq!(RingBuffer::<u32>::new(5).capacity(), 8);
    assert_eq!(RingBuffer::<u32>::new(8).capacity(), 8);
    assert_eq!(RingBuffer::<u32>::new(1000).capacity(), 1024);
}

#[test]
fn очередь_сообщает_о_заполнении_и_пустоте() {
    let mut ring = RingBuffer::new(4);
    let (mut tx, mut rx) = ring.split();
    assert_eq!(rx.try_pop(), None);
    for value in 0..4 {
        assert_eq!(tx.try_push(value), Ok(()));
    }
    assert_eq!(tx.try_push(99), Err(99));
    for value in 0..4 {
        assert_eq!(rx.try_pop(), Some(value));
    }
    assert_eq!(rx.try_pop(), None);
}

#[test]
fn очередь_ёмкости_один_работает() {
    let mut ring = RingBuffer::new(1);
    let (mut tx, mut rx) = ring.split();
    for value in 0..100 {
        assert_eq!(tx.try_push(value), Ok(()));
        assert_eq!(tx.try_push(value + 1000), Err(value + 1000));
        assert_eq!(rx.try_pop(), Some(value));
        assert_eq!(rx.try_pop(), None);
    }
}

#[test]
fn очередь_освобождает_оставшиеся_элементы() {
    let marker = std::sync::Arc::new(());
    {
        let mut ring = RingBuffer::new(8);
        let (mut tx, mut rx) = ring.split();
        for _ in 0..5 {
            assert!(tx.try_push(marker.clone()).is_ok());
        }
        drop(rx.try_pop());
    }
    assert_eq!(std::sync::Arc::strong_count(&marker), 1);
}

#[test]
fn передача_между_потоками_без_потерь_и_в_порядке() {
    for capacity in [1, 2, 5, 64, 1024] {
        let items = 200_000u64;
        let report = spsc::run(capacity, items);
        assert_eq!(report.popped, items);
        assert_eq!(report.sum, items * (items + 1) / 2);
        assert!(report.ordered);
        assert_eq!(report.capacity, capacity.next_power_of_two());
    }
}
