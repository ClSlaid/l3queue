use std::{
    borrow::BorrowMut,
    cell::RefCell,
    ptr,
    sync::{
        atomic::{AtomicPtr, AtomicUsize, Ordering},
        Arc,
    },
};

type NodePtr<T> = AtomicPtr<Node<T>>;

struct Node<T> {
    pub item: Option<T>,
    pub next: *mut Node<T>,
}

impl<T> Node<T> {
    pub fn new(item: T) -> Self {
        Self {
            item: Some(item),
            next: ptr::null_mut(),
        }
    }
    pub fn new_empty() -> Self {
        Self {
            item: None,
            next: ptr::null_mut(),
        }
    }
}

pub struct LinkedQueue<T> {
    // empty list, which is much more easier to implement
    len: AtomicUsize,
    head: NodePtr<T>,
    tail: NodePtr<T>,
}

impl<T> LinkedQueue<T> {
    pub fn new() -> Self {
        let head = AtomicPtr::new(Node::new_empty().borrow_mut());
        let tail = AtomicPtr::new(head.load(Ordering::SeqCst));
        Self {
            len: AtomicUsize::new(0),
            head,
            tail,
        }
    }
    pub fn is_empty(&self) -> bool {
        0 == self.len.load(Ordering::Relaxed)
    }

    pub fn enqueue(&self, item: T) {
        let mut new_node = Node::new(item);
        let node_ptr: *mut Node<T> = &mut new_node;
        let old_tail = self.tail.load(Ordering::Acquire);
        let tail_next = unsafe { (*old_tail).next };
        let mut tail_next = AtomicPtr::new(tail_next);
        while tail_next
            .compare_exchange(
                ptr::null_mut(),
                node_ptr,
                Ordering::Release,
                Ordering::Relaxed,
            )
            .is_err()
        {
            let mut next = tail_next.load(Ordering::Relaxed);
            loop {
                if unsafe { (*next).next.is_null() } {
                    break;
                } else {
                    next = unsafe { (*next).next };
                }
            }
            tail_next = AtomicPtr::from(next);
        }
        let _ =
            self.tail
                .compare_exchange(old_tail, node_ptr, Ordering::Release, Ordering::Relaxed);
        // finish insert, increase length;
        self.len.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn test_enqueue() {
    let q = LinkedQueue::new();
    q.enqueue(3);
}
