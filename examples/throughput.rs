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
    let c_lq_cnt = Arc::new(AtomicUsize::new(0));
    let p_lq_cnt1 = p_lq_cnt.clone();
    let c_lq_cnt1 = c_lq_cnt.clone();

    let p_mq_cnt = Arc::new(AtomicUsize::new(0));
    let c_mq_cnt = Arc::new(AtomicUsize::new(0));
    let p_mq_cnt1 = p_mq_cnt.clone();
    let c_mq_cnt1 = c_mq_cnt.clone();

    let p_lq = Arc::new(LinkedQueue::new());
    let c_lq = p_lq.clone();
    let p_mq = Arc::new(MutexQueue::new());
    let c_mq = p_mq.clone();

    let t1 = thread::spawn(move || {
        for i in 0u128.. {
            p_lq.push(i);
            p_lq_cnt1.fetch_add(1, Ordering::Release);
        }
    });
    let t2 = thread::spawn(move || loop {
        loop {
            if c_lq.pop().is_some() {
                c_lq_cnt1.fetch_add(1, Ordering::Release);
            }
        }
    });
    let t3 = thread::spawn(move || {
        for i in 0u128.. {
            p_mq.push(i);
            p_mq_cnt1.fetch_add(1, Ordering::Release);
        }
    });
    let t4 = thread::spawn(move || loop {
        loop {
            if c_mq.pop().is_some() {
                c_mq_cnt1.fetch_add(1, Ordering::Release);
            }
        }
    });

    println!("time,lq_produced,lq_consumed,mq_produced,mq_consumed");
    let mut now = Instant::now();
    while now <= ddl {
        let lq_p = p_lq_cnt.load(Ordering::Acquire);
        let lq_c = c_lq_cnt.load(Ordering::Acquire);
        let mq_p = p_mq_cnt.load(Ordering::Acquire);
        let mq_c = c_mq_cnt.load(Ordering::Acquire);
        println!(
            "{},{},{},{},{}",
            now.duration_since(begin).as_secs(),
            lq_p,
            lq_c,
            mq_p,
            mq_c,
        );
        thread::sleep(epoch);
        now = Instant::now();
    }
}
