use std::default::Default;
use std::mem;

pub trait CyclicQueueInterface {
    type VAL_TYPE;

    fn max_capacity(&self) -> usize;
    fn len(&self) -> usize;
    fn push(&mut self, val: Self::VAL_TYPE) -> Option<Self::VAL_TYPE>;
    fn get_relative(&self, i: usize) -> Option<&Self::VAL_TYPE>;
    fn get_back(&self) -> Option<&Self::VAL_TYPE>;
}

#[derive(Debug)]
pub struct CyclicQueue<T> {
    data: Vec<T>,
    len: usize,
    cap: usize,
    front: usize,
    back: usize,
}

pub struct CyclicQueueIterator<'a, T> {
    data: &'a CyclicQueue<T>,
    i: usize,
}

impl<T: Default + Clone> CyclicQueue<T> {
    pub fn new(cap: usize) -> CyclicQueue<T> {
        CyclicQueue {
            data: vec![T::default(); cap],
            len: 0,
            cap: cap,
            front: 0,
            back: 0,
        }
    }

    pub fn iter<'a>(&'a self) -> CyclicQueueIterator<'a, T> {
        CyclicQueueIterator { data: &self, i: 0 }
    }
}

impl<'a, T> Iterator for CyclicQueueIterator<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        self.i += 1;

        self.data.get_relative(self.i)
    }
}

impl<T> CyclicQueueInterface for CyclicQueue<T> {
    type VAL_TYPE = T;

    fn max_capacity(&self) -> usize {
        self.cap
    }

    fn len(&self) -> usize {
        self.len
    }

    // From a naming decision used by the ring_buffer
    // although I actual implement the interface correctly
    fn get_relative(&self, i: usize) -> Option<&T> {
        if i >= self.len {
            None
        } else {
            Some(&self.data[(self.front + i) % self.cap])
        }
    }

    fn get_back(&self) -> Option<&T> {
        if self.len == 0 {
            None
        } else {
            Some(&self.data[self.back])
        }
    }

    fn push(&mut self, val: T) -> Option<T> {
        if self.len == 0 {
            // special case for empty buffer
            self.len += 1;
            self.data[self.back] = val;

            None
        } else {
            self.back = (self.back + 1) % self.cap;

            if self.len != self.cap {
                self.len += 1;
            }

            if self.front == self.back {
                self.front = (self.front + 1) % self.cap;

                let temp = mem::replace(&mut self.data[self.back], val);

                Some(temp)
            } else {
                self.data[self.back] = val;

                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let queue: CyclicQueue<String> = CyclicQueue::new(5);

        assert_eq!(queue.data, vec! {"","","","",""});
        assert_eq!(queue.cap, 5);
        assert_eq!(queue.len, 0);
        assert_eq!(queue.front, 0);
        assert_eq!(queue.back, 0);
    }

    #[test]
    fn test_push() {
        let mut queue = CyclicQueue::new(5);

        let prev = queue.push("the");
        assert_eq!(queue.data, vec! {"the","","","",""});
        assert_eq!(queue.cap, 5);
        assert_eq!(queue.len, 1);
        assert_eq!(queue.front, 0);
        assert_eq!(queue.back, 0);
        assert_eq!(prev, None);

        let prev = queue.push("dog");
        assert_eq!(queue.data, vec! {"the","dog","","",""});
        assert_eq!(queue.cap, 5);
        assert_eq!(queue.len, 2);
        assert_eq!(queue.front, 0);
        assert_eq!(queue.back, 1);
        assert_eq!(prev, None);

        let prev = queue.push("jumps");
        assert_eq!(queue.data, vec! {"the","dog","jumps","",""});
        assert_eq!(queue.cap, 5);
        assert_eq!(queue.len, 3);
        assert_eq!(queue.front, 0);
        assert_eq!(queue.back, 2);
        assert_eq!(prev, None);

        let prev = queue.push("over");
        assert_eq!(queue.data, vec! {"the","dog","jumps","over",""});
        assert_eq!(queue.cap, 5);
        assert_eq!(queue.len, 4);
        assert_eq!(queue.front, 0);
        assert_eq!(queue.back, 3);
        assert_eq!(prev, None);

        let prev = queue.push("a");
        assert_eq!(queue.data, vec! {"the","dog","jumps","over","a"});
        assert_eq!(queue.cap, 5);
        assert_eq!(queue.len, 5);
        assert_eq!(queue.front, 0);
        assert_eq!(queue.back, 4);
        assert_eq!(prev, None);

        let prev = queue.push("white");
        assert_eq!(queue.data, vec! {"white","dog","jumps","over","a"});
        assert_eq!(queue.cap, 5);
        assert_eq!(queue.len, 5);
        assert_eq!(queue.front, 1);
        assert_eq!(queue.back, 0);
        assert_eq!(prev, Some("the"));

        let prev = queue.push("fence");
        assert_eq!(queue.data, vec! {"white","fence","jumps","over","a"});
        assert_eq!(queue.cap, 5);
        assert_eq!(queue.len, 5);
        assert_eq!(queue.front, 2);
        assert_eq!(queue.back, 1);
        assert_eq!(prev, Some("dog"));

        let prev = queue.push("this");
        assert_eq!(queue.data, vec! {"white","fence","this","over","a"});
        assert_eq!(queue.cap, 5);
        assert_eq!(queue.len, 5);
        assert_eq!(queue.front, 3);
        assert_eq!(queue.back, 2);
        assert_eq!(prev, Some("jumps"));

        let prev = queue.push("is");
        assert_eq!(queue.data, vec! {"white","fence","this","is","a"});
        assert_eq!(queue.cap, 5);
        assert_eq!(queue.len, 5);
        assert_eq!(queue.front, 4);
        assert_eq!(queue.back, 3);
        assert_eq!(prev, Some("over"));

        let prev = queue.push("some");
        assert_eq!(queue.data, vec! {"white","fence","this","is","some"});
        assert_eq!(queue.cap, 5);
        assert_eq!(queue.len, 5);
        assert_eq!(queue.front, 0);
        assert_eq!(queue.back, 4);
        assert_eq!(prev, Some("a"));

        // make sure crosses completely to check the issue
        // of making sure the front pointer wraps correctly
        // FIXED
        let prev = queue.push("words");
        assert_eq!(queue.data, vec! {"words","fence","this","is","some"});
        assert_eq!(queue.cap, 5);
        assert_eq!(queue.len, 5);
        assert_eq!(queue.front, 1);
        assert_eq!(queue.back, 0);
        assert_eq!(prev, Some("white"));
    }

    #[test]
    fn test_get_relative() {
        let mut queue = CyclicQueue::new(5);

        queue.push("the".to_string());
        assert_eq!(queue.data, vec! {"the","","","",""});
        let val = queue.get_relative(0);
        assert_eq!(val, Some(&"the".to_string()));
        let val = queue.get_relative(1);
        assert_eq!(val, None);
        let val = queue.get_relative(2);
        assert_eq!(val, None);
        let val = queue.get_relative(3);
        assert_eq!(val, None);
        let val = queue.get_relative(4);
        assert_eq!(val, None);
        let val = queue.get_relative(5);
        assert_eq!(val, None);
        let val = queue.get_relative(6);
        assert_eq!(val, None);

        queue.push("dog".to_string());
        assert_eq!(queue.data, vec! {"the","dog","","",""});
        let val = queue.get_relative(0);
        assert_eq!(val, Some(&"the".to_string()));
        let val = queue.get_relative(1);
        assert_eq!(val, Some(&"dog".to_string()));
        let val = queue.get_relative(2);
        assert_eq!(val, None);
        let val = queue.get_relative(3);
        assert_eq!(val, None);
        let val = queue.get_relative(4);
        assert_eq!(val, None);
        let val = queue.get_relative(5);
        assert_eq!(val, None);
        let val = queue.get_relative(6);
        assert_eq!(val, None);

        queue.push("jumps".to_string());
        assert_eq!(queue.data, vec! {"the","dog","jumps","",""});
        let val = queue.get_relative(0);
        assert_eq!(val, Some(&"the".to_string()));
        let val = queue.get_relative(1);
        assert_eq!(val, Some(&"dog".to_string()));
        let val = queue.get_relative(2);
        assert_eq!(val, Some(&"jumps".to_string()));
        let val = queue.get_relative(3);
        assert_eq!(val, None);
        let val = queue.get_relative(4);
        assert_eq!(val, None);
        let val = queue.get_relative(5);
        assert_eq!(val, None);
        let val = queue.get_relative(6);
        assert_eq!(val, None);

        queue.push("over".to_string());
        assert_eq!(queue.data, vec! {"the","dog","jumps","over",""});
        let val = queue.get_relative(0);
        assert_eq!(val, Some(&"the".to_string()));
        let val = queue.get_relative(1);
        assert_eq!(val, Some(&"dog".to_string()));
        let val = queue.get_relative(2);
        assert_eq!(val, Some(&"jumps".to_string()));
        let val = queue.get_relative(3);
        assert_eq!(val, Some(&"over".to_string()));
        let val = queue.get_relative(4);
        assert_eq!(val, None);
        let val = queue.get_relative(5);
        assert_eq!(val, None);
        let val = queue.get_relative(6);
        assert_eq!(val, None);

        queue.push("a".to_string());
        assert_eq!(queue.data, vec! {"the","dog","jumps","over","a"});
        let val = queue.get_relative(0);
        assert_eq!(val, Some(&"the".to_string()));
        let val = queue.get_relative(1);
        assert_eq!(val, Some(&"dog".to_string()));
        let val = queue.get_relative(2);
        assert_eq!(val, Some(&"jumps".to_string()));
        let val = queue.get_relative(3);
        assert_eq!(val, Some(&"over".to_string()));
        let val = queue.get_relative(4);
        assert_eq!(val, Some(&"a".to_string()));
        let val = queue.get_relative(5);
        assert_eq!(val, None);
        let val = queue.get_relative(6);
        assert_eq!(val, None);

        queue.push("white".to_string());
        assert_eq!(queue.data, vec! {"white","dog","jumps","over","a"});
        let val = queue.get_relative(0);
        assert_eq!(val, Some(&"dog".to_string()));
        let val = queue.get_relative(1);
        assert_eq!(val, Some(&"jumps".to_string()));
        let val = queue.get_relative(2);
        assert_eq!(val, Some(&"over".to_string()));
        let val = queue.get_relative(3);
        assert_eq!(val, Some(&"a".to_string()));
        let val = queue.get_relative(4);
        assert_eq!(val, Some(&"white".to_string()));
        let val = queue.get_relative(5);
        assert_eq!(val, None);
        let val = queue.get_relative(6);
        assert_eq!(val, None);

        queue.push("fence".to_string());
        assert_eq!(queue.data, vec! {"white","fence","jumps","over","a"});
        let val = queue.get_relative(0);
        assert_eq!(val, Some(&"jumps".to_string()));
        let val = queue.get_relative(1);
        assert_eq!(val, Some(&"over".to_string()));
        let val = queue.get_relative(2);
        assert_eq!(val, Some(&"a".to_string()));
        let val = queue.get_relative(3);
        assert_eq!(val, Some(&"white".to_string()));
        let val = queue.get_relative(4);
        assert_eq!(val, Some(&"fence".to_string()));
        let val = queue.get_relative(5);
        assert_eq!(val, None);
        let val = queue.get_relative(6);
        assert_eq!(val, None);
    }
}
