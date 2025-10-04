use crate::collection::vec::{MyVec, raw_val_iter::RawValIter, raw_vec::MyRawVec};
use std::iter::FusedIterator;
use std::mem;
use std::ptr;
use std::slice;

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

impl<T> FusedIterator for IntoIter<T> {}

impl<T> Drop for IntoIter<T> {
    fn drop(&mut self) {
        unsafe {
            let drop_array: *mut [T] = slice::from_raw_parts_mut(self.iter.start_mut(), self.len());
            ptr::drop_in_place(drop_array);
        }
    }
}

impl<T> IntoIterator for MyVec<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;
    fn into_iter(mut self) -> IntoIter<T> {
        unsafe {
            let iter = RawValIter::new(&mut self);

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
