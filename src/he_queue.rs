// based on crossbeam
// push with strict tail algorithm

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

pub struct HeQueue<T> {
    len: AtomicUsize,
    head: NodePtr<T>,
    tail: NodePtr<T>,
}

impl<T> Default for HeQueue<T> {
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

impl<T> HeQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn size(&self) -> usize {
        self.len.load(Ordering::SeqCst)
    }

    pub fn is_empty(&self) -> bool {
        0 == self.len.load(Ordering::SeqCst)
    }

    pub fn push(&self, data: T) {
        let guard = epoch::pin();

        let new_node = Owned::new(Node::new(data)).into_shared(&guard);

        let mut tail;
        unsafe {
            let null = Shared::null();
            loop {
                tail = self.tail.load(Ordering::Acquire, &guard);
                let tail_next = &(*tail.as_raw()).next;
                if tail_next
                    .compare_exchange(null, new_node, Ordering::AcqRel, Ordering::Relaxed, &guard)
                    .is_ok()
                {
                    break;
                }
                let tail_next = tail_next.load(Ordering::Acquire, &guard);
                let _ = self.tail.compare_exchange(
                    tail,
                    tail_next,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                    &guard,
                );
            }
        }
        let _ = self.tail.compare_exchange(
            tail,
            new_node,
            Ordering::Release,
            Ordering::Relaxed,
            &guard,
        );

        self.len.fetch_add(1, Ordering::SeqCst);
    }

    pub fn pop(&self) -> Option<T> {
        let mut data = None;
        if self.is_empty() {
            return data;
        }
        let guard = &epoch::pin();
        unsafe {
            loop {
                let head = self.head.load(Ordering::Acquire, guard);
                let mut next = (*head.as_raw()).next.load(Ordering::Acquire, guard);

                if next.is_null() {
                    return None;
                }

                if self
                    .head
                    .compare_exchange(head, next, Ordering::Release, Ordering::Relaxed, guard)
                    .is_ok()
                {
                    data = next.deref_mut().item.take();
                    guard.defer_destroy(head);
                    break;
                }
            }
        }
        self.len.fetch_sub(1, Ordering::SeqCst);
        data
    }
}

impl<T> Drop for HeQueue<T> {
    fn drop(&mut self) {
        while self.pop().is_some() {}
        let guard = &epoch::pin();
        unsafe {
            let h = self.head.load_consume(guard);
            guard.defer_destroy(h);
        }
    }
}

impl<T> HeQueue<T> {
    // used for debugging
    // walk through the queue and print each element's address
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
mod he_test {
    use std::{
        sync::{
            atomic::{AtomicI32, Ordering},
            Arc, Barrier,
        },
        thread,
    };

    use crate::he_queue::HeQueue;

    #[test]
    fn test_single() {
        let q = HeQueue::new();
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

        let p1 = Arc::new(HeQueue::new());
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
        let pad = 100_0000u128;

        let flag = Arc::new(AtomicI32::new(3));
        let flag1 = flag.clone();
        let flag2 = flag.clone();
        let flag3 = flag.clone();
        let p1 = Arc::new(HeQueue::new());
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

        let p1 = Arc::new(HeQueue::new());
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
