use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use l3queue::{lq::LinkedQueue, mutex_queue::MutexQueue};

fn main() {
    let begin = Instant::now();
    let du = Duration::from_secs(60);
    let epoch = Duration::from_secs(1);
    let ddl = begin + du;

    let p_lq_cnt = Arc::new(AtomicUsize::new(0));
    let p_lq_cnt1 = p_lq_cnt.clone();

    let p_mq_cnt = Arc::new(AtomicUsize::new(0));
    let p_mq_cnt1 = p_mq_cnt.clone();

    let p_lq = Arc::new(LinkedQueue::new());
    let p_mq = Arc::new(MutexQueue::new());

    let _t1 = thread::spawn(move || {
        for i in 0u128.. {
            p_lq.push(i);
            p_lq_cnt1.fetch_add(1, Ordering::Release);
        }
    });
    let _t2 = thread::spawn(move || {
        for i in 0u128.. {
            p_mq.push(i);
            p_mq_cnt1.fetch_add(1, Ordering::Release);
        }
    });

    println!("time,lq_produced,mq_produced,compare");
    let mut now = Instant::now();
    while now <= ddl {
        let lq_p = p_lq_cnt.load(Ordering::Acquire);
        let mq_p = p_mq_cnt.load(Ordering::Acquire);

        let p = (lq_p as f64 + 1f64) / (mq_p as f64 + 1f64);
        println!(
            "{},{},{},{}",
            now.duration_since(begin).as_secs(),
            lq_p,
            mq_p,
            p
        );
        thread::sleep(epoch);
        now = Instant::now();
    }
}
