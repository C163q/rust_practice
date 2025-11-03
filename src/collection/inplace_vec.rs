mod drain;
mod into_iter;

pub use drain::Drain;
pub use into_iter::IntoIter;

use std::borrow::{Borrow, BorrowMut};
use std::hash::{Hash, Hasher};
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::{cmp, ptr, slice};

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

    fn extend_from_iter<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for elem in iter {
            self.overflow_check();
            unsafe {
                let ptr = self.as_mut_ptr().add(self.len);
                ptr::write(ptr, elem);
                self.len += 1;
            }
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

impl<'a, const N: usize, T: Clone + 'a> InplaceVec<N, T> {
    fn extend_from_iter_ref<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        for refer in iter {
            self.overflow_check();
            unsafe {
                let ptr = self.as_mut_ptr().add(self.len());
                ptr::write(ptr, refer.clone());
                self.len += 1;
            }
        }
    }

    unsafe fn unchecked_extend_from_iter_ref<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        for refer in iter {
            unsafe {
                let ptr = self.as_mut_ptr().add(self.len());
                ptr::write(ptr, refer.clone());
                self.len += 1;
            }
        }
    }
}

impl<const N: usize, T: Clone> InplaceVec<N, T> {
    pub fn extend_from_slice(&mut self, slice: &[T]) {
        assert!(self.len() + slice.len() <= N, "InplaceVec overflow");
        unsafe {
            self.unchecked_extend_from_iter_ref(slice);
        }
    }
}

impl<const N: usize, T> Extend<T> for InplaceVec<N, T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.extend_from_iter(iter);
    }
}

impl<'a, const N: usize, T: Clone> Extend<&'a T> for InplaceVec<N, T> {
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        self.extend_from_iter_ref(iter);
    }
}

impl<const N: usize, T> FromIterator<T> for InplaceVec<N, T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut ret = InplaceVec::new();
        ret.extend_from_iter(iter);
        ret
    }
}

impl<const N: usize, T: Clone> Clone for InplaceVec<N, T> {
    fn clone(&self) -> Self {
        let mut vec = InplaceVec::new();
        unsafe { vec.unchecked_extend_from_iter_ref(self.as_slice()) };
        vec
    }

    fn clone_from(&mut self, source: &Self) {
        self.clear();
        unsafe { self.unchecked_extend_from_iter_ref(source) };
    }
}

impl<const N: usize, T: Clone> From<&[T]> for InplaceVec<N, T> {
    fn from(value: &[T]) -> Self {
        let mut vec = InplaceVec::new();
        vec.extend_from_slice(value);
        vec
    }
}

impl<const N: usize, T: Clone> From<&mut [T]> for InplaceVec<N, T> {
    fn from(value: &mut [T]) -> Self {
        Self::from(&*value)
    }
}

impl<const N: usize, T: Clone, const M: usize> From<&[T; M]> for InplaceVec<N, T> {
    fn from(value: &[T; M]) -> Self {
        assert!(M <= N, "InplaceVec overflow");
        Self::from(value.as_slice())
    }
}

impl<const N: usize, T: Clone, const M: usize> From<&mut [T; M]> for InplaceVec<N, T> {
    fn from(value: &mut [T; M]) -> Self {
        Self::from(value.as_slice())
    }
}

impl<const N: usize, T: PartialEq> PartialEq for InplaceVec<N, T> {
    fn eq(&self, other: &Self) -> bool {
        (**self).eq(&**other)
    }
}

impl<const N: usize, T: Eq> Eq for InplaceVec<N, T> {}

impl<const N: usize, T: PartialEq> PartialEq<[T]> for InplaceVec<N, T> {
    fn eq(&self, other: &[T]) -> bool {
        (**self).eq(other)
    }
}

impl<const N: usize, T: PartialEq> PartialEq<&[T]> for InplaceVec<N, T> {
    fn eq(&self, other: &&[T]) -> bool {
        (**self).eq(*other)
    }
}

impl<const N: usize, T: PartialEq, const M: usize> PartialEq<[T; M]> for InplaceVec<N, T> {
    fn eq(&self, other: &[T; M]) -> bool {
        (**self).eq(other)
    }
}

impl<const N: usize, T: PartialEq, const M: usize> PartialEq<&[T; M]> for InplaceVec<N, T> {
    fn eq(&self, other: &&[T; M]) -> bool {
        (**self).eq(*other)
    }
}

impl<const N: usize, T: PartialOrd> PartialOrd<InplaceVec<N, T>> for InplaceVec<N, T> {
    fn partial_cmp(&self, other: &InplaceVec<N, T>) -> Option<cmp::Ordering> {
        (**self).partial_cmp(&**other)
    }
}

impl<const N: usize, T: Ord> Ord for InplaceVec<N, T> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        (**self).cmp(&**other)
    }
}

impl<const N: usize, T> AsMut<[T]> for InplaceVec<N, T> {
    fn as_mut(&mut self) -> &mut [T] {
        self
    }
}

impl<const N: usize, T> AsMut<InplaceVec<N, T>> for InplaceVec<N, T> {
    fn as_mut(&mut self) -> &mut InplaceVec<N, T> {
        self
    }
}

impl<const N: usize, T> AsRef<[T]> for InplaceVec<N, T> {
    fn as_ref(&self) -> &[T] {
        self
    }
}

impl<const N: usize, T> AsRef<InplaceVec<N, T>> for InplaceVec<N, T> {
    fn as_ref(&self) -> &InplaceVec<N, T> {
        self
    }
}

impl<const N: usize, T: Hash> Hash for InplaceVec<N, T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        <T as Hash>::hash_slice(self, state);
    }
}

impl<const N: usize, T> Borrow<[T]> for InplaceVec<N, T> {
    fn borrow(&self) -> &[T] {
        self
    }
}

impl<const N: usize, T> BorrowMut<[T]> for InplaceVec<N, T> {
    fn borrow_mut(&mut self) -> &mut [T] {
        self
    }
}
