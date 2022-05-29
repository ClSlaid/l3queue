use std::{collections::LinkedList, sync::Mutex};
pub struct LockQueue<T> {
    inner: Mutex<LinkedList<T>>,
}

impl<T> Default for LockQueue<T> {
    fn default() -> Self {
        let inner = Mutex::new(LinkedList::new());
        Self { inner }
    }
}

impl<T> LockQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        let guard = self.inner.lock().unwrap();
        guard.is_empty()
    }

    pub fn push(&self, item: T) {
        let mut guard = self.inner.lock().unwrap();
        guard.push_back(item);
    }

    pub fn pop(&self) -> Option<T> {
        let mut guard = self.inner.lock().unwrap();
        guard.pop_front()
    }
}

#[cfg(test)]
mod test {
    use std::{
        sync::{
            atomic::{AtomicI32, Ordering},
            Arc, Barrier,
        },
        thread,
    };

    use super::LockQueue;
    #[test]
    fn test_single() {
        let q = LockQueue::new();
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

        let p1 = Arc::new(LockQueue::new());
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
        let pad = 10000;

        let flag = Arc::new(AtomicI32::new(3));
        let flag1 = flag.clone();
        let flag2 = flag.clone();
        let flag3 = flag.clone();
        let p1 = Arc::new(LockQueue::new());
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
