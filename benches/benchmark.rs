use criterion::{black_box, criterion_group, criterion_main, Criterion};
use l3queue::{lq::LinkedQueue, mutex_queue::MutexQueue};

fn single_insert_lockless_benchmark(c: &mut Criterion) {
    let q = LinkedQueue::new();
    c.bench_function("single insert lockless", |b| {
        b.iter(|| q.push(black_box(1)));
    });
}

fn single_insert_lock_benchmark(c: &mut Criterion) {
    let q = MutexQueue::new();
    c.bench_function("single insert lock", |b| b.iter(|| q.push(black_box(1))));
}

fn lockless_throughput_benchmark(c: &mut Criterion) {
    let q = LinkedQueue::new();
    c.bench_function("lockless push and pop", |b| {
        b.iter(|| {
            q.push(black_box(1));
            black_box(q.pop());
        })
    });
}

fn lock_throughput_benchmark(c: &mut Criterion) {
    let q = MutexQueue::new();
    c.bench_function("lock push and pop", |b| {
        b.iter(|| {
            q.push(black_box(1));
            black_box(q.pop());
        })
    });
}
criterion_group!(
    benches,
    single_insert_lockless_benchmark,
    single_insert_lock_benchmark,
    lockless_throughput_benchmark,
    lock_throughput_benchmark,
);

criterion_main!(benches);
