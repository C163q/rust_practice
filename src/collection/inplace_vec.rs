mod into_iter;

pub use into_iter::IntoIter;

use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::{ptr, slice};

/// 类似[`Vec`]，但是预先分配好N个元素的缓冲区，且不会动态扩容。
///
/// ```rust
/// use rust_practice::collection::inplace_vec::InplaceVec;
///
/// let mut vec = InplaceVec::<2, _>::new();
/// vec.push(1);
/// vec.push(2);
/// let ptr = vec.as_mut_ptr();
/// drop(vec);
/// assert_eq!(unsafe { *(ptr.offset(1)) }, 2);
/// ```
///
/// `InplaceVec`的内存是自动释放的，因此在使用`*(ptr.offset(1))`时，
/// 内存仍然有效，而[`i32`]的[`drop`]什么都不做，因此这段代码完全合
/// 法。
#[derive(Debug)]
pub struct InplaceVec<const N: usize, T> {
    buf: [MaybeUninit<T>; N],
    len: usize,
}

impl<T, const N: usize> InplaceVec<N, T> {
    pub const fn new() -> Self {
        Self {
            // 在此我们使用inline const pattern (RFC 2920)，这样T就无须是Copy的。
            // 可见[rust-lang/rust#76001](https://github.com/rust-lang/rust/issues/76001)
            buf: [const { MaybeUninit::uninit() }; N],
            len: 0,
        }
    }

    #[inline]
    pub const fn as_slice(&self) -> &[T] {
        unsafe { slice::from_raw_parts(self.as_ptr(), self.len) }
    }

    #[inline]
    pub const fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { slice::from_raw_parts_mut(self.as_mut_ptr(), self.len) }
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        N
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
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

    #[inline]
    pub const fn as_ptr(&self) -> *const T {
        // cast操作是安全的，因为MaybeUninit<T>和T在内存布局上是相同的
        self.buf.as_ptr().cast()
    }

    #[inline]
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

    pub fn clear(&mut self) {
        let drop_array: *mut [T] = self.as_mut_slice();

        unsafe {
            self.len = 0;
            ptr::drop_in_place(drop_array);
        }
    }
}

impl<T, const N: usize> Default for InplaceVec<N, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Drop for InplaceVec<N, T> {
    fn drop(&mut self) {
        unsafe {
            ptr::drop_in_place(self.as_mut_slice());
        }
    }
}

impl<T, const N: usize> Deref for InplaceVec<N, T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T, const N: usize> DerefMut for InplaceVec<N, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}
