use criterion::{black_box, criterion_group, criterion_main, Criterion};
use l3queue::{lock_queue::LockQueue, lq::LinkedQueue};

fn single_insert_lockless_benchmark(c: &mut Criterion) {
    let q = LinkedQueue::new();
    c.bench_function("single insert lockless", |b| {
        b.iter(|| q.push(black_box(1)));
    });
}

fn single_insert_lock_benchmark(c: &mut Criterion) {
    let q = LockQueue::new();
    c.bench_function("single insert lock", |b| b.iter(|| q.push(black_box(1))));
}

criterion_group!(
    benches,
    single_insert_lockless_benchmark,
    single_insert_lock_benchmark
);

criterion_main!(benches);
