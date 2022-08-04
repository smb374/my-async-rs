use std::time::Duration;

use criterion::{
    criterion_group, criterion_main, measurement::Measurement, BatchSize, BenchmarkId, Criterion,
    SamplingMode, Throughput,
};
use crossbeam_utils::sync::WaitGroup;

mod my_queue_test {
    use std::thread;

    use concurrent_ringbuf::{Ringbuf, Steal, Stealer};
    use crossbeam_utils::sync::WaitGroup;

    pub fn setup(input_size: usize, threads: usize) -> (WaitGroup, WaitGroup) {
        let start = WaitGroup::new();
        let end = WaitGroup::new();
        let ringbuf: Ringbuf<usize> = Ringbuf::new(input_size);

        for idx in 0..threads {
            work(ringbuf.stealer(), start.clone(), end.clone(), idx);
        }

        for _ in 0..input_size {
            let _ = ringbuf.push(1);
        }

        (start, end)
    }

    fn work(stealer: Stealer<usize>, start: WaitGroup, end: WaitGroup, idx: usize) {
        let name = format!("stealer_{}", idx);
        let builder = thread::Builder::new();
        let _ = builder.name(name).spawn(move || {
            start.wait(); // sync starting point
            loop {
                match stealer.steal() {
                    Steal::Empty => break,
                    _ => {}
                }
            }
            end.wait();
        });
    }
}

mod crossbeam_test {
    use std::thread;

    use crossbeam_deque::{Steal, Stealer, Worker};
    use crossbeam_utils::sync::WaitGroup;

    pub fn setup(input_size: usize, threads: usize) -> (WaitGroup, WaitGroup) {
        let start = WaitGroup::new();
        let end = WaitGroup::new();
        let worker: Worker<usize> = Worker::new_lifo();

        for idx in 0..threads {
            work(worker.stealer(), start.clone(), end.clone(), idx);
        }

        for _ in 0..input_size {
            worker.push(1);
        }

        (start, end)
    }

    fn work(stealer: Stealer<usize>, start: WaitGroup, end: WaitGroup, idx: usize) {
        let name = format!("stealer_{}", idx);
        let builder = thread::Builder::new();
        let _ = builder.name(name).spawn(move || {
            start.wait(); // sync starting point
            loop {
                match stealer.steal() {
                    Steal::Empty => break,
                    _ => {}
                }
            }
            end.wait();
        });
    }
}

fn bench(start: WaitGroup, end: WaitGroup) {
    start.wait(); // sync worker thread start point
    end.wait(); // sync worker thread end point
}

fn steal_bench(c: &mut Criterion<impl Measurement>, threads: usize) {
    let mut group = c.benchmark_group(format!("Time impact by items with {} threads", &threads));
    group.sampling_mode(SamplingMode::Auto);
    group.measurement_time(Duration::from_secs(10));
    let steps: Vec<u64> = (10000..=100000).step_by(20000).collect();
    for &size in steps.iter() {
        group.throughput(Throughput::Elements(size));

        group.bench_with_input(BenchmarkId::new("my queue", size), &size, |b, &s| {
            b.iter_batched(
                || my_queue_test::setup(s as usize, threads),
                |(start, end)| bench(start, end),
                BatchSize::SmallInput,
            )
        });
        group.bench_with_input(BenchmarkId::new("crossbeam-deque", size), &size, |b, &s| {
            b.iter_batched(
                || crossbeam_test::setup(s as usize, threads),
                |(start, end)| bench(start, end),
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn thread_bench(c: &mut Criterion<impl Measurement>, input_size: usize) {
    let mut group = c.benchmark_group(format!("Time impact by threads with {} items", &input_size));
    group.sampling_mode(SamplingMode::Auto);
    group.measurement_time(Duration::from_secs(10));
    let steps: Vec<usize> = [2, 4, 8, 16].to_vec();
    for &threads in steps.iter() {
        group.throughput(Throughput::Elements(input_size as u64));

        group.bench_with_input(BenchmarkId::new("my queue", threads), &threads, |b, &t| {
            b.iter_batched(
                || my_queue_test::setup(input_size, t),
                |(start, end)| bench(start, end),
                BatchSize::SmallInput,
            )
        });
        group.bench_with_input(
            BenchmarkId::new("crossbeam-deque", threads),
            &threads,
            |b, &t| {
                b.iter_batched(
                    || crossbeam_test::setup(input_size, t),
                    |(start, end)| bench(start, end),
                    BatchSize::SmallInput,
                )
            },
        );
    }
    group.finish();
}

pub fn steal_bench_2(c: &mut Criterion<impl Measurement>) {
    steal_bench(c, 2);
}

pub fn steal_bench_4(c: &mut Criterion<impl Measurement>) {
    steal_bench(c, 4);
}

pub fn steal_bench_8(c: &mut Criterion<impl Measurement>) {
    steal_bench(c, 8);
}

pub fn steal_bench_16(c: &mut Criterion<impl Measurement>) {
    steal_bench(c, 16);
}

pub fn thread_bench_10000(c: &mut Criterion<impl Measurement>) {
    thread_bench(c, 10000)
}

pub fn thread_bench_30000(c: &mut Criterion<impl Measurement>) {
    thread_bench(c, 30000)
}

pub fn thread_bench_50000(c: &mut Criterion<impl Measurement>) {
    thread_bench(c, 50000)
}

pub fn thread_bench_70000(c: &mut Criterion<impl Measurement>) {
    thread_bench(c, 70000)
}

pub fn thread_bench_90000(c: &mut Criterion<impl Measurement>) {
    thread_bench(c, 90000)
}

criterion_group!(
    name = thread;
    config = Criterion::default();
    targets = steal_bench_2, steal_bench_4, steal_bench_8, steal_bench_16
);

criterion_group!(
    name = item;
    config = Criterion::default();
    targets = thread_bench_10000, thread_bench_30000, thread_bench_50000, thread_bench_70000, thread_bench_90000
);

criterion_main!(thread, item);
