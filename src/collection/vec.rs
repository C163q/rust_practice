mod drain;
mod into_iter;
mod raw_val_iter;
mod raw_vec;
mod vec_macro;

use raw_vec::MyRawVec;
use std::borrow::{Borrow, BorrowMut};
use std::hash::{Hash, Hasher};
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;
use std::slice;
use std::{cmp, ptr};

pub use drain::Drain;
pub use into_iter::IntoIter;

#[derive(Debug)]
pub struct MyVec<T> {
    buf: MyRawVec<T>,
    len: usize,
}

impl<T> MyVec<T> {
    #[inline]
    pub const fn as_mut_ptr(&mut self) -> *mut T {
        self.buf.ptr().as_ptr()
    }

    #[inline]
    pub const fn as_ptr(&self) -> *const T {
        self.buf.ptr().as_ptr()
    }

    #[inline]
    pub const fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { slice::from_raw_parts_mut(self.as_mut_ptr(), self.len) }
    }

    #[inline]
    pub const fn as_slice(&self) -> &[T] {
        unsafe { slice::from_raw_parts(self.as_ptr(), self.len) }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.buf.cap()
    }

    fn extend_from_iter<I: Iterator<Item = T>>(&mut self, mut iter: I) {
        while let Some(elem) = iter.next() {
            if self.len == self.capacity() {
                let (lower, _) = iter.size_hint();
                self.reserve(lower.saturating_add(1));
            }
            unsafe {
                let ptr = self.as_mut_ptr().add(self.len);
                ptr::write(ptr, elem);
                self.len += 1;
            }
        }
    }

    /// ## Safety
    ///
    /// - `new_len`不应该超过`capacity()`
    /// - `old_len..new_len`的元素必须被初始化
    #[inline]
    pub unsafe fn set_len(&mut self, new_len: usize) {
        self.len = new_len;
    }

    #[inline]
    pub fn new() -> Self {
        MyVec {
            buf: MyRawVec::new(),
            len: 0,
        }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        MyVec {
            buf: MyRawVec::with_capacity(capacity),
            len: 0,
        }
    }

    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        unsafe {
            // SAFETY:
            // 此处使用了filter来保证new_cap不会超过`isize::MAX`
            self.buf.reserve_exact(
                self.len
                    .checked_add(additional)
                    .filter(|&new_cap| new_cap <= isize::MAX as usize)
                    .expect("Allocation too large"),
            );
        }
    }

    /// 详细说明见[`MyVec::drop`]
    #[inline]
    pub fn clear(&mut self) {
        let drop_array: *mut [T] = self.as_mut_slice();

        unsafe {
            // `drop_array`所指向的内容不包含`self.len`，因此此处使用`self.len`
            // 是可行的。
            //
            // 此时使用`self.len = 0`来防止在调用[`ptr::drop_in_place`]时`panic`，
            // 导致Unwinding时再次调用`drop`，从而二次释放内存。
            self.len = 0;

            // 对`[T]`使用`drop_in_place`会对其中的每个元素调用`drop`。
            ptr::drop_in_place(drop_array);
        }
    }

    /// 源自The Rustonomicon
    ///
    /// 实现push方法其实非常简单，一般有以下步骤：
    ///
    /// 1. 确定是否需要增加容量
    /// 2. 写入元素到尾部
    /// 3. 大小增加1
    ///
    /// 在写入元素的时候不应该访问未初始化内存的内容，例如
    /// `self.as_mut_ptr()[self.len] = elem`就是错误的，因为它尝试访问
    /// 未分配内存的内容并可能会试图调用[`drop`]。
    ///
    /// 使用[`ptr::write`]可以直接写入目标内存而不访问或者调用其
    /// [`drop`]。
    pub fn push(&mut self, elem: T) {
        if self.len == self.capacity() {
            self.grow();
        }

        unsafe {
            ptr::write(self.as_mut_ptr().add(self.len), elem);
        }

        // Can't fail, we'll OOM first.
        self.len += 1;
    }

    /// 源自The Rustonomicon
    ///
    /// 对于pop来说，rust并不允许我们直接移动指针所指向的值，因为
    /// 这会导致指向的内存空间变为未初始化的。
    ///
    /// 因此我们需要首先使用[`ptr::read`]读取内存中的元素，获取带
    /// 有所有权的值，然后直接无视这部分内存，将其作为逻辑上未初
    /// 始化的空间。
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            unsafe { Some(ptr::read(self.as_mut_ptr().add(self.len))) }
        }
    }

    #[inline]
    fn grow(&mut self) {
        self.buf.grow();
    }

    /// 源自The Rustonomicon
    ///
    /// 要执行insert的逻辑，首先需要将待插入位置后面的所有元素都向
    /// 后移动一个位置。此时我们可以使用[`ptr::copy`]函数，这个函数
    /// 相当于C中的`memmove`函数，可以用于处理源位置和目标位置有重
    /// 叠的情况。同样，也有一个函数[`ptr::copy_nonoverlapping`]，
    /// 相当于C中的`memcpy`函数，不能处理重叠的情况，但会更加高效。
    /// 此处大部分情况下都会有重叠，因此我们使用`ptr::copy`。
    pub fn insert(&mut self, index: usize, elem: T) {
        // 注意：当插入的`index`为`self.len`时，意味着插入到所有元素后面，
        // 这是合理的，且等价于`push`。new_layout
        assert!(index <= self.len, "index out of bounds");
        if self.len == self.capacity() {
            self.grow();
        }

        unsafe {
            ptr::copy(
                self.as_ptr().add(index),
                self.as_mut_ptr().add(index + 1),
                self.len - index,
            );
            ptr::write(self.as_mut_ptr().add(index), elem);
        }

        self.len += 1;
    }

    /// 源自The Rustonomicon
    ///
    /// remove是insert相反的操作，我们仍然使用[`ptr::copy`]，但这次
    /// 向前移动一个位置。
    ///
    /// 我们无须关心移动之后尾部后面那个位置，把它当成逻辑上未初始
    /// 化的空间即可。
    pub fn remove(&mut self, index: usize) -> T {
        // 注意：此处`index`不应等于`self.len`，因为不能移除所有元素之后的
        // 那个位置，那边是可能是未初始化或者未被分配的内存空间。
        assert!(index < self.len, "index out of bounds");
        unsafe {
            self.len -= 1;
            let result = ptr::read(self.as_mut_ptr().add(index));
            ptr::copy(
                self.as_mut_ptr().add(index + 1),
                self.as_mut_ptr().add(index),
                self.len - index,
            );
            result
        }
    }

    /// # Safety
    /// - ptr must have been allocated using the global allocator,
    ///   such as via the alloc::alloc function.
    /// - T needs to have the same alignment as what ptr was allocated with.
    ///   (T having a less strict alignment is not sufficient, the alignment
    ///   really needs to be equal to satisfy the dealloc requirement that
    ///   memory must be allocated and deallocated with the same layout.)
    /// - The size of T times the capacity (ie. the allocated size in bytes)
    ///   needs to be the same size as the pointer was allocated with. (Because
    ///   similar to alignment, dealloc must be called with the same layout size.)
    /// - length needs to be less than or equal to capacity.
    /// - The first length values must be properly initialized values of type T.
    /// - capacity needs to be the capacity that the pointer was allocated with.
    /// - The allocated size in bytes must be no larger than isize::MAX. See
    ///   the safety documentation of pointer::offset.
    #[inline]
    pub unsafe fn from_parts(ptr: NonNull<T>, length: usize, capacity: usize) -> Self {
        Self {
            buf: unsafe { MyRawVec::from_parts(ptr, capacity) },
            len: length,
        }
    }

    /// ## Safety
    /// TODO: Finish safety doc
    pub unsafe fn from_raw_parts(ptr: *mut T, length: usize, capacity: usize) -> Self {
        Self {
            buf: unsafe { MyRawVec::from_raw_parts(ptr, capacity) },
            len: length,
        }
    }
}

impl<'a, T: Clone + 'a> MyVec<T> {
    fn extend_from_iter_ref<I: Iterator<Item = &'a T>>(&mut self, mut iter: I) {
        while let Some(refer) = iter.next() {
            if self.len == self.capacity() {
                let (lower, _) = iter.size_hint();
                self.reserve(lower.saturating_add(1));
            }
            unsafe {
                let ptr = self.as_mut_ptr().add(self.len());
                ptr::write(ptr, refer.clone());
                self.len += 1;
            }
        }
    }
}

impl<T: Clone> MyVec<T> {
    #[allow(unused)]
    pub fn extend_from_slice(&mut self, other: &[T]) {
        let remain = self.capacity() - self.len();
        let needs = other.len();
        if needs > remain {
            self.reserve(unsafe { needs.unchecked_sub(remain) });
        }
        unsafe { self.unchecked_extend_from_slice(other) }
    }

    /// ## Safety
    ///
    /// - [`MyVec`]的`capacity`必须足够容纳下整个`&[T]`
    unsafe fn unchecked_extend_from_slice(&mut self, slice: &[T]) {
        let iter = slice.iter();
        for refer in iter {
            unsafe {
                let ptr = self.as_mut_ptr().add(self.len());
                ptr::write(ptr, refer.clone());
                self.len += 1;
            }
        }
    }
}

impl<T> Default for MyVec<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

/// 源自The Rustonomicon
///
/// 为了让[`MyVec`]可以索引、切片等操作，并且能够类型转换为
/// `&[T]`，我们可以实现`Deref<Target=[T]>`。
///
/// [`slice::from_raw_parts`]能够很好地处理ZST和`size == 0`
/// 的情况，因此无须特别讨论。
///
/// 在[`deref`]函数中，隐含了`&Self::Target`的声明周期与`&self`
/// 相同。见[`The Rustonomicon`](https://doc.rust-lang.org/nomicon/lifetime-elision.html)
/// 也因此，我们保证返回的slice永远不会超过自身的声明周期。
impl<T> Deref for MyVec<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

/// 源自The Rustonomicon
///
/// 与[`Deref`]类似，不做赘述。
impl<T> DerefMut for MyVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

/// 源自The Rustonomicon
///
/// 由于在调用完[`MyVec::drop`]之后，会自动调用其所有字段的
/// [`drop`]，因此无须关系分配的内存，仅仅需要drop其中存储的
/// 所有元素即可。
///
/// 我们可以通过[`mem::needs_drop`]来检查是否需要对其中存储的
/// 元素调用drop，这样就可以对不需要drop的情况进行优化。然而
/// 编译器往往有能力会对这种情况进行优化，我们一般只需要在编
/// 译器无法优化的时候显式地写出即可。
///
/// 在此处，我们将其中的所有元素`pop`即可。编译器能够合理地优
/// 化下面的代码，因此无须使用`mem::needs_drop`。
///
/// 注：现已修改为直接调用[`MyVec::clear`]。
impl<T> Drop for MyVec<T> {
    fn drop(&mut self) {
        unsafe {
            ptr::drop_in_place(self.as_mut_slice());
        }
    }
    // `MyRawVec`会自动帮助释放内存空间
}

impl<T: Clone> Clone for MyVec<T> {
    fn clone(&self) -> Self {
        let raw = MyRawVec::<T>::with_capacity(self.len);
        let ptr = raw.ptr().as_ptr();

        for (idx, element) in self.iter().enumerate() {
            unsafe {
                let ptr = ptr.add(idx);
                ptr::write(ptr, element.clone());
            }
        }

        MyVec {
            buf: raw,
            len: self.len,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        if self.capacity() < source.len() {
            self.reserve(source.len() - self.capacity());
        }
        self.clear();

        let ptr = self.as_mut_ptr();
        for (idx, refer) in source.iter().enumerate() {
            unsafe {
                ptr::write(ptr.add(idx), refer.clone());
            }
        }

        unsafe {
            self.set_len(source.len());
        }
    }
}

impl<T: PartialEq> PartialEq for MyVec<T> {
    fn eq(&self, other: &Self) -> bool {
        (**self).eq(&**other)
    }
}

impl<T> Eq for MyVec<T> where T: Eq {}

impl<T: PartialEq> PartialEq<[T]> for MyVec<T> {
    fn eq(&self, other: &[T]) -> bool {
        (**self).eq(other)
    }
}

impl<T: PartialEq> PartialEq<&[T]> for MyVec<T> {
    fn eq(&self, other: &&[T]) -> bool {
        (**self).eq(*other)
    }
}

impl<T: PartialEq, const N: usize> PartialEq<[T; N]> for MyVec<T> {
    fn eq(&self, other: &[T; N]) -> bool {
        (**self).eq(other)
    }
}

impl<T: PartialEq, const N: usize> PartialEq<&[T; N]> for MyVec<T> {
    fn eq(&self, other: &&[T; N]) -> bool {
        (**self).eq(*other)
    }
}

impl<T> Extend<T> for MyVec<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.extend_from_iter(iter.into_iter());
    }
}

impl<'a, T: Clone> Extend<&'a T> for MyVec<T> {
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        self.extend_from_iter_ref(iter.into_iter());
    }
}

impl<T> FromIterator<T> for MyVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let (lower, _) = iter.size_hint();
        let mut ret = Self::with_capacity(lower);
        ret.extend_from_iter(iter);
        ret
    }
}

impl<T: Clone> From<&[T]> for MyVec<T> {
    fn from(value: &[T]) -> Self {
        let mut ret = MyVec::with_capacity(value.len());
        unsafe {
            ret.unchecked_extend_from_slice(value);
        }
        ret
    }
}

impl<T: Clone> From<&mut [T]> for MyVec<T> {
    fn from(value: &mut [T]) -> Self {
        Self::from(&*value)
    }
}

impl<T: Clone, const N: usize> From<&[T; N]> for MyVec<T> {
    fn from(value: &[T; N]) -> Self {
        Self::from(value.as_slice())
    }
}

impl<T: Clone, const N: usize> From<&mut [T; N]> for MyVec<T> {
    fn from(value: &mut [T; N]) -> Self {
        Self::from(value.as_mut_slice())
    }
}

impl<T> From<Vec<T>> for MyVec<T> {
    fn from(value: Vec<T>) -> Self {
        // 阻止`Vec`被`drop`，因为我们要接管其内存
        let mut value = ManuallyDrop::new(value);
        
        // `Vec::into_raw_parts`仍然是unstable的特性，在此我们不使用
        let (ptr, len, cap) = (
            value.as_mut_ptr(),
            value.len(),
            value.capacity(),
        );

        unsafe { MyVec::from_raw_parts(ptr, len, cap) }
    }
}

impl<T> From<MyVec<T>> for Vec<T> {
    fn from(value: MyVec<T>) -> Self {
        // 阻止`MyVec`被`drop`，因为我们要接管其内存
        let mut value = ManuallyDrop::new(value);

        let (ptr, len, cap) = (
            value.as_mut_ptr(),
            value.len(),
            value.capacity(),
        );

        unsafe { Vec::from_raw_parts(ptr, len, cap) }
    }
}

impl<T: PartialOrd> PartialOrd<MyVec<T>> for MyVec<T> {
    fn partial_cmp(&self, other: &MyVec<T>) -> Option<cmp::Ordering> {
        (**self).partial_cmp(&**other)
    }
}

impl<T: Ord> Ord for MyVec<T> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        (**self).cmp(&**other)
    }
}

impl<T> AsMut<[T]> for MyVec<T> {
    fn as_mut(&mut self) -> &mut [T] {
        self
    }
}

impl<T> AsMut<MyVec<T>> for MyVec<T> {
    fn as_mut(&mut self) -> &mut MyVec<T> {
        self
    }
}

impl<T> AsRef<[T]> for MyVec<T> {
    fn as_ref(&self) -> &[T] {
        self
    }
}

impl<T> AsRef<MyVec<T>> for MyVec<T> {
    fn as_ref(&self) -> &MyVec<T> {
        self
    }
}

impl<T: Hash> Hash for MyVec<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        <T as Hash>::hash_slice(self, state);
    }
}

impl<T> Borrow<[T]> for MyVec<T> {
    fn borrow(&self) -> &[T] {
        self
    }
}

impl<T> BorrowMut<[T]> for MyVec<T> {
    fn borrow_mut(&mut self) -> &mut [T] {
        self
    }
}
