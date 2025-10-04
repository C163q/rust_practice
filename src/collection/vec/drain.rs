use std::ptr;
use std::{marker::PhantomData, ops::RangeBounds, ptr::NonNull};

use crate::collection;
use crate::collection::vec::{MyVec, raw_val_iter::RawValIter};

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
/// [`std::mem::forget`]是安全的代码，但比如现在`Drain`迭代到
/// 了一半，现在`MyVec`中一半的空间是未初始化的，另外一半仍然
/// 有效，接着我对`Drain`调用了`mem::forget`，这导致我们没有
/// 机会执行析构函数中的逻辑！也就是说，元素没有机会补位，我
/// 们没有设置正确长度的机会！
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
///
/// ## 关于成员的考量（自己写）
///
/// 在此处，我们不能使用`&'a MyVec<T>`，因为根据[`std::ptr`](https://doc.rust-lang.org/std/ptr/index.html#pointer-to-reference-conversion)
/// 文档中提到的：
///
/// > When creating a mutable reference, then while this reference exists,
/// > the memory it points to must not get accessed (read or written) through
/// > any other pointer or reference not derived from this reference.
///
/// 因此我们必须使用一个[`NonNull`]。此外，我们还需要绑定一个
/// 生命周期，这个生命周期不能超过引用的`MyVec`的生命周期，因
/// 此我们使用`PhantomData<&'a MyVec>`。
pub struct Drain<'a, T: 'a> {
    _marker: PhantomData<&'a MyVec<T>>,
    vec: NonNull<MyVec<T>>,
    iter: RawValIter<T>,
    before_len: usize,
    after_len: usize,
    old_len: usize,
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

        let vec_ref = unsafe { self.vec.as_mut() };
        let vec_ptr = vec_ref.as_mut_ptr();
        // `self.vec.as_mut()`在此处结束生命周期，所以使用*mut T是安全的

        let before_len = self.before_len;
        let after_len = self.after_len;

        // SAFETY:
        // 此处无论是`before_len`还是`old_len - after_len`都是不超过
        // `old_len`的，因此不会到分配空间之外。
        //
        // 此外，`vec_len - after_len`的结果为创建时的`range.end`，不可能
        // 下溢。
        //
        // 我们始终保证`after_len`记录了`range.end`之后（包括本身）的元素
        // 个数，所以使用[`copy`]不会越界。`before_len + after_len`正好是
        // 剩下的元素个数，我们用该值恢复[`MyVec`]的长度。
        unsafe {
            let hole_begin = vec_ptr.add(before_len);
            let hole_end = vec_ptr.add(self.old_len - after_len);

            ptr::copy(hole_end, hole_begin, after_len);
            self.vec.as_mut().set_len(before_len + after_len);
        };
    }
}

impl<T> MyVec<T> {
    /// 此处我们先暂时不考虑传入范围作为参数，仅仅是实现整个[`MyVec`]
    /// 都被drain的情况。
    pub fn drain<R: RangeBounds<usize>>(&mut self, range: R) -> Drain<'_, T> {
        let range = collection::slice::range(range, ..self.len);
        let iter = unsafe { RawValIter::new(&self[range.clone()]) };

        let old_len = self.len();
        let before_len = range.start;
        let after_len = self.len() - range.end;

        // 这是为了保证在使用`mem::forget`之后，仍然是安全的。如果`Drain`
        // 被forget了，我们就让整个`MyVec`都泄露了。
        self.len = 0;

        Drain {
            old_len,
            before_len,
            after_len,
            iter,
            vec: NonNull::from_mut(self),
            _marker: PhantomData,
        }
    }
}
