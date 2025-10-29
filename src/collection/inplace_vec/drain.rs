use std::{
    marker::PhantomData,
    ops::RangeBounds,
    ptr::{self, NonNull},
};

use crate::collection::{self, inplace_vec::InplaceVec};

pub struct Drain<'a, const N: usize, T> {
    _marker: PhantomData<&'a T>,
    vec: NonNull<InplaceVec<N, T>>,
    start: usize,
    end: usize,
    before_len: usize,
    after_len: usize,
    old_len: usize,
}

impl<'a, const N: usize, T> Iterator for Drain<'a, N, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start >= self.end {
            None
        } else {
            let item = unsafe { self.vec.as_mut().buf[self.start].assume_init_read() };
            self.start += 1;
            Some(item)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, const N: usize, T> DoubleEndedIterator for Drain<'a, N, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.start >= self.end {
            None
        } else {
            self.end -= 1;
            let item = unsafe { self.vec.as_mut().buf[self.end].assume_init_read() };
            Some(item)
        }
    }
}

impl<'a, const N: usize, T> ExactSizeIterator for Drain<'a, N, T> {
    fn len(&self) -> usize {
        self.end - self.start
    }
}

impl<'a, const N: usize, T> Drop for Drain<'a, N, T> {
    fn drop(&mut self) {
        for _ in &mut *self {}

        let buf_ptr = unsafe { self.vec.as_mut().buf.as_mut_ptr() };

        let before_len = self.before_len;
        let after_len = self.after_len;

        unsafe {
            let hole_begin = buf_ptr.add(before_len);
            let hole_end = buf_ptr.add(self.old_len - after_len);

            ptr::copy(hole_end, hole_begin, after_len);
            self.vec.as_mut().len = before_len + after_len;
        }
    }
}

impl<const N: usize, T> InplaceVec<N, T> {
    pub fn drain<R: RangeBounds<usize>>(&mut self, range: R) -> Drain<'_, N, T> {
        let old_len = self.len();
        let range = collection::slice::range(range, ..old_len);

        let before_len = range.start;
        let after_len = old_len - range.end;

        self.len = 0;

        Drain {
            _marker: PhantomData,
            old_len,
            vec: NonNull::from_mut(self),
            start: range.start,
            end: range.end,
            before_len,
            after_len,
        }
    }
}
