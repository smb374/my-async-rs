use concurrent_ringbuf::{Ringbuf, Steal, Stealer};
use crossbeam_utils::{
    sync::WaitGroup,
    thread::{scope, Scope},
};

fn setup(s: &Scope, input_size: usize, threads: usize) -> (WaitGroup, WaitGroup) {
    let start = WaitGroup::new();
    let end = WaitGroup::new();
    let ringbuf: Ringbuf<usize> = Ringbuf::new(input_size);

    for idx in 0..threads {
        work(s, ringbuf.stealer(), start.clone(), end.clone(), idx);
    }

    for _ in 0..input_size {
        let _ = ringbuf.push(1);
    }

    (start, end)
}

fn work(s: &Scope, stealer: Stealer<usize>, start: WaitGroup, end: WaitGroup, idx: usize) {
    let name = format!("stealer_{}", idx);
    s.builder()
        .name(name)
        .spawn(move |_| {
            start.wait();
            loop {
                match stealer.steal() {
                    Steal::Empty => break,
                    _ => {}
                }
            }
            end.wait();
        })
        .unwrap();
}

fn bench(start: WaitGroup, end: WaitGroup) {
    start.wait();
    end.wait();
}

fn main() {
    let size = 1_000_000_00;
    scope(|s| {
        let (start, end) = setup(s, size, 8);
        bench(start, end);
    })
    .unwrap();
}
