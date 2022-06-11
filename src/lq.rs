// a lockless empty linked list based queue
// suffering from UAF and ABA problems

use std::{
    ptr,
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

type NodePtr<T> = AtomicPtr<Node<T>>;

struct Node<T> {
    pub item: Option<T>,
    pub next: NodePtr<T>,
}

impl<T> Node<T> {
    pub fn new(item: T) -> Self {
        Self {
            item: Some(item),
            next: AtomicPtr::new(ptr::null_mut()),
        }
    }
    pub fn new_empty() -> Self {
        Self {
            item: None,
            next: AtomicPtr::new(ptr::null_mut()),
        }
    }
}

/// WARNING:
/// LinkedQueue does not fix ABA problem and UAF bug in multi-consumer scenarios
pub struct LinkedQueue<T> {
    // empty list, which is much more easier to implement
    len: AtomicUsize,
    head: NodePtr<T>,
    tail: NodePtr<T>,
}

impl<T> Default for LinkedQueue<T> {
    fn default() -> Self {
        let header = Box::new(Node::new_empty());
        let head = AtomicPtr::from(Box::into_raw(header));
        let tail = AtomicPtr::new(head.load(Ordering::SeqCst));
        Self {
            len: AtomicUsize::new(0),
            head,
            tail,
        }
    }
}

impl<T> LinkedQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.len
            .compare_exchange(0, 0, Ordering::Release, Ordering::Relaxed)
            .is_ok()
    }

    pub fn push(&self, item: T) {
        let new_node = Box::new(Node::new(item));
        let node_ptr: *mut Node<T> = Box::into_raw(new_node);

        let old_tail = self.tail.load(Ordering::Acquire);
        unsafe {
            let mut tail_next = &(*old_tail).next;
            while tail_next
                .compare_exchange(
                    ptr::null_mut(),
                    node_ptr,
                    Ordering::Release,
                    Ordering::Relaxed,
                )
                .is_err()
            {
                let mut tail = tail_next.load(Ordering::Acquire);

                // step to tail
                loop {
                    let nxt = (*tail).next.load(Ordering::Acquire);
                    if nxt.is_null() {
                        break;
                    }
                    tail = nxt;
                }

                tail_next = &(*tail).next;
            }
        }
        let _ =
            self.tail
                .compare_exchange(old_tail, node_ptr, Ordering::Release, Ordering::Relaxed);
        // finish insert, increase length;
        self.len.fetch_add(1, Ordering::SeqCst);
    }

    pub fn pop(&self) -> Option<T> {
        let mut data = None;
        if self.is_empty() {
            return data;
        }
        unsafe {
            let mut head;
            loop {
                head = self.head.load(Ordering::Acquire);
                let next = (*head).next.load(Ordering::Acquire);

                if next.is_null() {
                    return None;
                }

                if self
                    .head
                    .compare_exchange(head, next, Ordering::Release, Ordering::Relaxed)
                    .is_ok()
                {
                    data = (*next).item.take();
                    break;
                }
            }
            // drop `head`
            let _ = Box::from_raw(head);
        };
        self.len.fetch_sub(1, Ordering::SeqCst);

        data
    }
}

impl<T> Drop for LinkedQueue<T> {
    fn drop(&mut self) {
        while self.pop().is_some() {}
        let h = self.head.load(Ordering::SeqCst);
        unsafe {
            // drop `h`
            Box::from_raw(h);
        }
    }
}

#[cfg(test)]
mod lq_test {
    use std::{
        sync::{
            atomic::{AtomicI32, Ordering},
            Arc, Barrier,
        },
        thread,
    };

    use crate::lq::LinkedQueue;

    #[test]
    fn test_single() {
        let q = LinkedQueue::new();
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

        let p1 = Arc::new(LinkedQueue::new());
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
        let p1 = Arc::new(LinkedQueue::new());
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
}
