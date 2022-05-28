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
        0 == self.len.load(Ordering::Relaxed)
    }

    pub fn enqueue(&self, item: T) {
        // println!("begin enqueue");
        // println!("head: {:?}, tail: {:?}", self.head, self.tail);
        // link_runner(&self.head);
        let new_node = Box::new(Node::new(item));
        let node_ptr: *mut Node<T> = Box::into_raw(new_node);

        let old_tail = self.tail.load(Ordering::Acquire);
        // println!("old_tail: {:?}", old_tail);
        let mut tail_next = unsafe { &(*old_tail).next };
        // println!("tail_next: {:?}", tail_next.load(Ordering::SeqCst));
        while tail_next
            .compare_exchange(
                ptr::null_mut(),
                node_ptr,
                Ordering::Release,
                Ordering::Relaxed,
            )
            .is_err()
        {
            // println!("CAS fail");
            let mut next = tail_next.load(Ordering::Acquire);
            // println!("next: {:?}", next);
            loop {
                if unsafe { (*next).next.load(Ordering::Relaxed).is_null() } {
                    break;
                } else {
                    next = unsafe { (*next).next.load(Ordering::Relaxed) };
                }
            }
            // println!("step next to: {:?}", next);
            tail_next = unsafe { &(*next).next };
        }
        // println!("tail_next changed: {:?}", tail_next.load(Ordering::SeqCst));
        // println!("node_ptr: {:?}", node_ptr);
        if self
            .tail
            .compare_exchange(old_tail, node_ptr, Ordering::Release, Ordering::Relaxed)
            .is_ok()
        {
            // println!("hit!")
        }
        // finish insert, increase length;
        self.len.fetch_add(1, Ordering::SeqCst);
        // println!("head: {:?}, tail: {:?}", self.head, self.tail);
        // link_runner(&self.head);
        // println!("enqueue finish")
    }

    pub fn dequeue(&self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let mut head = self.head.load(Ordering::Acquire);
        // println!("head: {:?}", head);
        let mut next = unsafe { (*head).next.load(Ordering::Acquire) };
        // println!("next: {:?}", next);
        while self
            .head
            .compare_exchange(head, next, Ordering::Release, Ordering::Relaxed)
            .is_err()
        {
            head = self.head.load(Ordering::Acquire);
            next = unsafe { (*head).next.load(Ordering::Relaxed) };
        }
        unsafe {
            Box::from_raw(head);
        }
        self.len.fetch_sub(1, Ordering::SeqCst);
        unsafe { (*next).item.take() }
    }
}

impl<T> Drop for LinkedQueue<T> {
    fn drop(&mut self) {
        while self.dequeue().is_some() {}
        let h = self.head.load(Ordering::SeqCst);
        unsafe {
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
        q.enqueue(1);
        q.enqueue(1);
        q.enqueue(4);
        q.enqueue(5);
        q.enqueue(1);
        q.enqueue(4);
        assert_eq!(q.dequeue(), Some(1));
        assert_eq!(q.dequeue(), Some(1));
        assert_eq!(q.dequeue(), Some(4));
        assert_eq!(q.dequeue(), Some(5));
        assert_eq!(q.dequeue(), Some(1));
        assert_eq!(q.dequeue(), Some(4));
    }

    #[test]
    fn test_send() {
        let q1 = Arc::new(LinkedQueue::new());
        let q2 = q1.clone();
        let ba1 = Arc::new(Barrier::new(2));
        let ba2 = ba1.clone();
        let t1 = thread::spawn(move || {
            for i in 0..100 {
                q1.enqueue(i);
            }
            ba1.wait();
        });
        ba2.wait();
        let mut i = 0;
        while let Some(got) = q2.dequeue() {
            assert_eq!(got, i);
            i += 1;
        }
        let _ = t1.join();
    }

    #[test]
    fn test_mpsc() {
        let pad = 100;

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
                p1.enqueue(i);
            }
            flag1.fetch_sub(1, Ordering::SeqCst);
        });
        let t2 = thread::spawn(move || {
            for i in pad..(2 * pad) {
                p2.enqueue(i);
            }
            flag2.fetch_sub(1, Ordering::SeqCst);
        });
        let t3 = thread::spawn(move || {
            for i in (2 * pad)..(3 * pad) {
                p3.enqueue(i);
            }
            flag3.fetch_sub(1, Ordering::SeqCst);
        });

        let mut sum = 0;
        while flag.load(Ordering::SeqCst) != 0 {
            if let Some(num) = c.dequeue() {
                sum += num;
            }
        }

        t1.join().unwrap();
        t2.join().unwrap();
        t3.join().unwrap();
        assert_eq!(sum, (0..(3 * pad)).sum());
    }
}
