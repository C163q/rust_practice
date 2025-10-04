use std::{mem::MaybeUninit, ptr};

/// 类似[`Vec`]，但是预先分配好N个元素的缓冲区，且不会动态扩容。
#[derive(Debug)]
pub struct InplaceVec<T, const N: usize> {
    buf: [MaybeUninit<T>; N],
    len: usize,
}

impl<T, const N: usize> InplaceVec<T, N> {
    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn overflow_check(&self) {
        if self.len >= N {
            panic!("InplaceVec overflow");
        }
    }

    pub fn push(&mut self, value: T) {
        self.overflow_check();
        self.buf[self.len].write(value);
        self.len += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            unsafe { Some(self.buf[self.len].assume_init_read()) }
        }
    }

    pub const fn as_ptr(&self) -> *const T {
        self.buf.as_ptr().cast()
    }

    pub const fn as_mut_ptr(&mut self) -> *mut T {
        self.buf.as_mut_ptr().cast()
    }

    pub fn insert(&mut self, index: usize, value: T) {
        self.overflow_check();
        assert!(index <= self.len, "InplaceVec insert index out of bounds");

        unsafe {
            ptr::copy(
                self.as_ptr().add(index),
                self.as_mut_ptr().add(index + 1),
                self.len - index,
            )
        }
        self.buf[index].write(value);

        self.len += 1;
    }

    pub fn remove(&mut self, index: usize) -> T {
        assert!(index < self.len, "InplaceVec remove index out of bounds");
        unsafe {
            self.len -= 1;
            let result = self.buf[index].assume_init_read();
            ptr::copy(
                self.as_ptr().add(index + 1),
                self.as_mut_ptr().add(index),
                self.len - index,
            );
            result
        }
    }
}
