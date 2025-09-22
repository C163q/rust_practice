mod raw_vec;
mod raw_val_iter;

use std::marker::PhantomData;
use std::mem::{self};
use std::ops::{Deref, DerefMut};
use std::ptr::{self};
use std::{slice};
use raw_vec::MyRawVec;
use raw_val_iter::RawValIter;

#[derive(Debug)]
pub struct MyVec<T> {
    buf: MyRawVec<T>,
    len: usize,
}

impl<T> MyVec<T> {
    #[inline]
    pub fn as_mut_ptr(&self) -> *mut T {
        self.buf.ptr().as_ptr()
    }

    #[inline]
    pub fn as_ptr(&self) -> *const T {
        self.buf.ptr().as_ptr()
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

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn new() -> Self {
        MyVec {
            buf: MyRawVec::new(),
            len: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        MyVec {
            buf: MyRawVec::with_capacity(capacity),
            len: 0,
        }
    }

    pub fn reserve(&mut self, additional: usize) {
        unsafe {
            // SAFETY:
            // 此处使用了filter来保证new_cap不会超过`isize::MAX`
            self.buf.reserve_exact(
                self.len.checked_add(additional)
                        .filter(|&new_cap| new_cap <= isize::MAX as usize)
                        .expect("Allocation too large")
            );
        }
    }

    /// 详细说明见[`MyVec::drop`]
    pub fn clear(&mut self) {
        while self.pop().is_some() {}
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
                self.as_mut_ptr().add(index),
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
}

impl<'a, T: Clone + 'a> MyVec<T> {
    fn extend_from_iter_ref<I: Iterator<Item = &'a T>>(&mut self, mut iter: I) {
        while let Some(refer) = iter.next() {
            if self.len == self.capacity() {
                let (lower, _) = iter.size_hint();
                self.reserve(lower.saturating_add(1));
            }
            unsafe {
                let ptr = self.as_mut_ptr().add(self.len);
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
        unsafe { slice::from_raw_parts(self.as_mut_ptr(), self.len) }
    }
}

/// 源自The Rustonomicon
///
/// 与[`Deref`]类似，不做赘述。
impl<T> DerefMut for MyVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { slice::from_raw_parts_mut(self.as_mut_ptr(), self.len) }
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
        self.clear();
    }
    // `MyRawVec`会自动帮助释放内存空间
}

impl<T> Clone for MyVec<T>
    where T: Clone {
    fn clone(&self) -> Self {
        let raw = MyRawVec::<T>::with_capacity(self.len);
        let mut ptr = raw.ptr().as_ptr();

        for elem in self {
            unsafe {
                ptr::write(ptr, elem.clone());
                ptr = ptr.add(1);
            }
        }

        MyVec {
            buf: raw,
            len: self.len,
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

impl<T: PartialEq, const N: usize> PartialEq<[T; N]> for MyVec<T> {
    fn eq(&self, other: &[T; N]) -> bool {
        (**self).eq(other)
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

/// 源自The Rustonomicon
///
/// 既然我们已经为[`MyVec`]实现了[`Deref`]和[`DerefMut`]
/// 自然就表示我们已经可以使用`[T]`的`iter`和`iter_mut`方
/// 法了。但是我们仍然没有`into_iter`方法，这表明需要我们
/// 自己去实现。
///
/// ## 实现方式
///
/// [`IntoIter`]按值消费`MyVec`，并依序按值产出其中的元素。
/// 为了获取元素的所有权，需要让`IntoIter`获取`MyVec`分配
/// 的空间，因此需要一个`MyRawVec`。由于[`Vec`]的特性，
/// `IntoIter`也应该是[`DoubleEndedIterator`]，为支持该特
/// 性，可以使用两个指针指向开始和超尾处。
///
/// ```text
///  start  end
///    ↓     ↓
/// +-+-+-+-+-+
/// |U|1|2|3|U|
/// +-+-+-+-+-+
/// U: 被移出的元素（逻辑上未初始化）
/// ```
///
/// 当两个指针相遇时，迭代结束：
///
/// ```text
///    start
///      ↓
/// +-+-+-+-+-+
/// |U|U|U|U|U|
/// +-+-+-+-+-+
///      ↑
///     end
/// ```
///
/// 这是由于向前迭代或者向后迭代都是基于同一块内存的，并
/// 且调用向前迭代和向后迭代的顺序是任意的，因此我们不应
/// 当继续迭代。见[`DoubleEndedIterator`]
///
/// > It is important to note that both back and forth
/// > work on the same range, and do not cross: iteration
/// > is over when they meet in the middle.
///
/// ## 处理ZST
///
/// 而对于ZST来说，由于其指针的偏移永远为0，这就导致按照
/// 上面的表示方法，其start指针和end指针永远指向同一个地
/// 方，因此迭代器永远会产出[`None`]。
///
/// 目前的解决方案是对ZST特别讨论，start的保持不变，将
/// `start as usize - end as usize`定义为剩余的元素个数，
/// 向后迭代则`start as usize + 1`，向前迭代则`end as usize - 1`。
/// 由于我们指定了ZST的容量为[`isize::MAX`]，因此`end`一
/// 定不会溢出。
///
/// ## [`RawValIter`]抽象
///
/// 考虑到接下来[`Drain`]的逻辑中，也存在双向迭代，因此可
/// 以将这部分的内容放到[`RawValIter`]中。
pub struct IntoIter<T> {
    // 我们并不使用`MyRawVec`中的任何逻辑，我们只是希望保有缓冲区，
    // 并在使用完后自动释放内存空间。
    _buf: MyRawVec<T>,
    iter: RawValIter<T>,
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.iter.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<T> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<T> {
        self.iter.next_back()
    }
}

impl<T> ExactSizeIterator for IntoIter<T> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<T> Drop for IntoIter<T> {
    fn drop(&mut self) {
        // 由于[`IntoIterator::into_iter`]会自动为实现了[`Iterator`]
        // 的类型实现，因此下面的循环可以执行。他会获取剩余元素的所
        // 有权，然后drop。
        for _ in &mut *self {}
    }
}

impl<T> IntoIterator for MyVec<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;
    fn into_iter(self) -> IntoIter<T> {
        unsafe {
            let iter = RawValIter::new(&self);

            // 获取`MyVec`中分配的空间的所有权并阻止其drop
            let buf = ptr::read(&self.buf);
            mem::forget(self);

            IntoIter { iter, _buf: buf }
        }
    }
}

impl<'a, T> IntoIterator for &'a MyVec<T> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut MyVec<T> {
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

/// 源自The Rustonomicon
///
/// [`Drain`]是通过[`MyVec::drain`]返回的，其主要功能是移除
/// 指定范围的[`MyVec`]子序列，并返回该范围的[`DoubleEndedIterator`]。
///
/// `Drain`在执行[`drop`]的时候，会drop掉剩余未消费的元素，
/// 并使得原[`MyVec`]中空缺的位置被后续的元素补位。
///
/// 使用[`MyVec::drain`]会让`Drain`拥有`MyVec`的可变引用，这
/// 使得在`Drain`的生命周期内，无法获取`MyVec`的引用。
///
/// 编写`Drain`的迭代基本可以套用[`RawValIter`]，我们将值移出
/// 缓冲区之后，就将那块内存当作未初始化的内存。编写析构的时
/// 候，只需要把未移出缓冲区的元素全部移出缓冲区，然后将`MyRawVec`
/// 中后面的元素向前移动补位，并设置合适的长度即可。
///
/// 根据Rustonomicon，这样编写`Drain`时，可能会存在一个问题：
///
/// [`mem::forget`]是安全的代码，但比如现在`Drain`迭代到了一
/// 半，现在`MyVec`中一半的空间是未初始化的，另外一半仍然有效，
/// 接着我对`Drain`调用了`mem::forget`，这导致我们没有机会执
/// 行析构函数中的逻辑！也就是说，元素没有机会补位，我们没有
/// 设置正确长度的机会！
///
/// 此时，我们访问`MyVec`中的元素时，就可能会访问那些未初始化
/// 的内存。更糟的是，在对`MyVec`执行`drop`时，会对其中的部分
/// 内容二次析构。无论那种情况，都不应该暴露到safe代码中。
///
/// 我们确实可以在每次移出元素的同时，让后面的元素向前移动，
/// 这样即使调用了`mem::forget`，也不会导致上述问题。但这会导
/// 致性能下降。
///
/// 我们可以在创建`Drain`时，将引用的`MyVec`的大小设置为0，而
/// 在`drop`时赋予正确的值。这使得如果调用了`mem::forget`，就
/// 无法访问`MyVec`中的元素，但其中的元素也不会被`drop`。可是
/// 既然`mem::forget`属于安全的代码，那么这也必然是安全的。
///
/// 我们将泄露(leak)导致更多的泄露称为泄露放大(leak amplification)。
pub struct Drain<'a, T: 'a> {
    // 在此，我们需要绑定生命周期，我们需要使用`&'a mut MyVec<T>`，
    // 因为在语义上，我们拥有一个`MyVec`的引用，但并不使用它。
    vec: PhantomData<&'a mut MyVec<T>>,
    iter: RawValIter<T>,
}

impl<'a, T> Iterator for Drain<'a, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.iter.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, T> DoubleEndedIterator for Drain<'a, T> {
    fn next_back(&mut self) -> Option<T> {
        self.iter.next_back()
    }
}

impl<'a, T> Drop for Drain<'a, T> {
    fn drop(&mut self) {
        // 这会自动drop剩余元素
        for _ in &mut *self {}
        // 由于我们暂时没有传入范围作为参数的逻辑，因此`self.len`
        // 必然为0，也就没有必要再去修改了。
    }
}

impl<T> MyVec<T> {
    /// 此处我们先暂时不考虑传入范围作为参数，仅仅是实现整个[`MyVec`]
    /// 都被drain的情况。
    pub fn drain(&mut self) -> Drain<'_, T> {
        let iter = unsafe { RawValIter::new(self) };

        // 这是为了保证在使用`mem::forget`之后，仍然是安全的。如果`Drain`
        // 被forget了，我们就让整个`MyVec`都泄露了。并且，我们反正总归要
        // 将其长度设置为0，为什么不现在就做。
        self.len = 0;

        Drain {
            iter,
            vec: PhantomData,
        }
    }
}

