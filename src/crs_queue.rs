use std::{
    io::Write,
    sync::atomic::{AtomicUsize, Ordering},
};

use crossbeam::epoch;
use epoch::{Atomic, Owned, Shared};

type NodePtr<T> = Atomic<Node<T>>;
struct Node<T> {
    pub item: Option<T>,
    pub next: NodePtr<T>,
}

impl<T> Node<T> {
    pub fn new_empty() -> Self {
        Self {
            item: None,
            next: Atomic::null(),
        }
    }

    pub fn new(data: T) -> Self {
        Self {
            item: Some(data),
            next: Atomic::null(),
        }
    }
}

pub struct CrsQueue<T> {
    len: AtomicUsize,
    head: NodePtr<T>,
    tail: NodePtr<T>,
}

impl<T> Default for CrsQueue<T> {
    fn default() -> Self {
        let head = Atomic::new(Node::new_empty());
        let tail = head.clone();
        Self {
            len: AtomicUsize::new(0),
            head,
            tail,
        }
    }
}

impl<T> CrsQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn size(&self) -> usize {
        self.len.load(Ordering::SeqCst)
    }

    pub fn is_empty(&self) -> bool {
        self.len
            .compare_exchange(0, 0, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
    }

    pub fn push(&self, data: T) {
        let guard = epoch::pin();

        let new_node = Owned::new(Node::new(data)).into_shared(&guard);

        let old_tail = self.tail.load(Ordering::Acquire, &guard);
        unsafe {
            let mut tail_next = &(*old_tail.as_raw()).next;
            while tail_next
                .compare_exchange(
                    Shared::null(),
                    new_node,
                    Ordering::Release,
                    Ordering::Relaxed,
                    &guard,
                )
                .is_err()
            {
                let mut tail = tail_next.load(Ordering::Acquire, &guard).as_raw();

                // step to tail
                loop {
                    let nxt = (*tail).next.load(Ordering::Acquire, &guard);
                    if nxt.is_null() {
                        break;
                    }
                    tail = nxt.as_raw();
                }

                tail_next = &(*tail).next;
            }
        }
        let _ = self.tail.compare_exchange(
            old_tail,
            new_node,
            Ordering::Release,
            Ordering::Relaxed,
            &guard,
        );

        self.len.fetch_add(1, Ordering::SeqCst);
    }

    pub fn pop(&self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let guard = &epoch::pin();
        let mut data;
        unsafe {
            loop {
                let head = self.head.load(Ordering::Acquire, guard);
                let mut next = (*head.as_raw()).next.load(Ordering::Acquire, guard);

                if next.is_null() {
                    return None;
                }

                data = next.deref_mut().item.take();
                let next = next.into_owned();

                if self
                    .head
                    .compare_exchange(head, next, Ordering::Release, Ordering::Relaxed, &guard)
                    .is_ok()
                {
                    guard.defer_destroy(head);
                    break;
                }
            }
        }
        self.len.fetch_sub(1, Ordering::SeqCst);
        data
    }
}

impl<T> Drop for CrsQueue<T> {
    fn drop(&mut self) {
        while self.pop().is_some() {}
        let guard = epoch::pin();
        unsafe {
            let _h = self.head.load_consume(&guard).into_owned();
        }
    }
}

impl<T> CrsQueue<T> {
    pub fn walk(&self) {
        let _ = std::io::stdout().flush();
        let guard = epoch::pin();
        let mut start = self.head.load(Ordering::Acquire, &guard);

        let mut actual_len = 0;
        while !start.is_null() {
            unsafe {
                print!("-> {:?}", start.as_raw());
                let _ = std::io::stdout().flush();
                start = (*start.as_raw()).next.load(Ordering::Acquire, &guard);
            }
            actual_len += 1;
        }
        println!(" size:{} actual: {}", self.size(), actual_len - 1);
    }
}

#[cfg(test)]
mod cq_test {
    use std::{
        sync::{
            atomic::{AtomicI32, Ordering},
            Arc, Barrier,
        },
        thread,
    };

    use crate::crs_queue::CrsQueue;

    #[test]
    fn test_single() {
        let q = CrsQueue::new();
        q.push(1);
        q.push(1);
        q.push(4);
        q.push(5);
        q.push(1);
        q.push(4);
        assert_eq!(q.pop(), Some(1));
        assert_eq!(q.pop(), Some(1));
        assert_eq!(q.pop(), Some(4));
        assert_eq!(q.pop(), Some(5));
        assert_eq!(q.pop(), Some(1));
        assert_eq!(q.pop(), Some(4));
    }

    #[test]
    fn test_concurrent_send() {
        let pad = 100000_u128;

        let p1 = Arc::new(CrsQueue::new());
        let p2 = p1.clone();
        let c = p1.clone();
        let ba1 = Arc::new(Barrier::new(3));
        let ba2 = ba1.clone();
        let ba3 = ba1.clone();
        let t1 = thread::spawn(move || {
            for i in 0..pad {
                p1.push(i);
            }
            ba1.wait();
        });
        let t2 = thread::spawn(move || {
            for i in pad..(2 * pad) {
                p2.push(i);
            }
            ba2.wait();
        });
        // receive after send is finished
        ba3.wait();
        let mut sum = 0;
        while let Some(got) = c.pop() {
            sum += got;
        }
        let _ = t1.join();
        let _ = t2.join();
        assert_eq!(sum, (0..(2 * pad)).sum())
    }

    #[test]
    fn test_mpsc() {
        let pad = 10_0000u128;

        let flag = Arc::new(AtomicI32::new(3));
        let flag1 = flag.clone();
        let flag2 = flag.clone();
        let flag3 = flag.clone();
        let p1 = Arc::new(CrsQueue::new());
        let p2 = p1.clone();
        let p3 = p1.clone();
        let c = p1.clone();

        let t1 = thread::spawn(move || {
            for i in 0..pad {
                p1.push(i);
            }
            flag1.fetch_sub(1, Ordering::SeqCst);
        });
        let t2 = thread::spawn(move || {
            for i in pad..(2 * pad) {
                p2.push(i);
            }
            flag2.fetch_sub(1, Ordering::SeqCst);
        });
        let t3 = thread::spawn(move || {
            for i in (2 * pad)..(3 * pad) {
                p3.push(i);
            }
            flag3.fetch_sub(1, Ordering::SeqCst);
        });

        let mut sum = 0;
        while flag.load(Ordering::SeqCst) != 0 || !c.is_empty() {
            if let Some(num) = c.pop() {
                sum += num;
            }
        }

        t1.join().unwrap();
        t2.join().unwrap();
        t3.join().unwrap();
        assert_eq!(sum, (0..(3 * pad)).sum());
    }

    #[test]
    fn test_mpmc() {
        let pad = 10_0000u128;

        let flag = Arc::new(AtomicI32::new(3));
        let flag_c = flag.clone();
        let flag1 = flag.clone();
        let flag2 = flag.clone();
        let flag3 = flag.clone();

        let p1 = Arc::new(CrsQueue::new());
        let p2 = p1.clone();
        let p3 = p1.clone();
        let c1 = p1.clone();
        let c2 = p1.clone();

        let producer1 = thread::spawn(move || {
            for i in 0..pad {
                p1.push(i);
            }
            flag1.fetch_sub(1, Ordering::SeqCst);
        });
        let producer2 = thread::spawn(move || {
            for i in pad..(2 * pad) {
                p2.push(i);
            }
            flag2.fetch_sub(1, Ordering::SeqCst);
        });
        let producer3 = thread::spawn(move || {
            for i in (2 * pad)..(3 * pad) {
                p3.push(i);
            }
            flag3.fetch_sub(1, Ordering::SeqCst);
        });

        let consumer = thread::spawn(move || {
            let mut sum = 0;
            while flag_c.load(Ordering::SeqCst) != 0 || !c2.is_empty() {
                if let Some(num) = c2.pop() {
                    sum += num;
                }
            }
            sum
        });

        let mut sum = 0;
        while flag.load(Ordering::SeqCst) != 0 || !c1.is_empty() {
            if let Some(num) = c1.pop() {
                sum += num;
            }
        }

        producer1.join().unwrap();
        producer2.join().unwrap();
        producer3.join().unwrap();

        let s = consumer.join().unwrap();
        sum += s;
        assert_eq!(sum, (0..(3 * pad)).sum());
    }
}
