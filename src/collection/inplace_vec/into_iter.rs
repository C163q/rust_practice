use core::slice;
use std::{
    iter::FusedIterator,
    mem::{self, MaybeUninit},
    ptr,
};

use crate::collection::inplace_vec::InplaceVec;

/// 在此处，[`InplaceVec`]的迭代逻辑和[`MyVec`]的迭代逻辑完全相同，
/// 唯一值得注意的是，两者的drop逻辑不同。`MyVec`需要手动释放，因此
/// 我们在`MyVec`的`IntoIter`中使用了[`MyRawVec`]来管理内存。而在`InplaceVec`
/// 中，由于内存是自动释放的，因此我们不需要一个`RawVec`来管理内存。
/// 但这并不表明我们不需要关心原来的buffer，如果我们把IntoIter实现
/// 为如下的样子：
/// ```rust,ignore
/// pub struct WrongIntoIter<T> {
///     iter: RawValIter<T>,
/// }
///
/// impl<T> Iterator for WrongIntoIter<T> {
///     type Item = T;
///     fn next(&mut self) -> Option<Self::Item> {
///         self.iter.next()
///     }
/// }
///
/// impl<const N: usize, T> IntoIterator for InplaceVec<N, T> {
///     type Item = T;
///     type IntoIter = WrongIntoIter<T>;
///     fn into_iter(mut self) -> Self::IntoIter {
///         unsafe {
///             let iter = RawValIter::new(self.as_mut_slice());
///             mem::forget(self);
///             WrongIntoIter { iter }
///         }
///     }
/// }
///
/// impl<T> WrongIntoIter<T> {
///     pub fn ptr(&self) -> *const T {
///         self.iter.start()
///     }
/// }
///
/// let mut iter = {
///     let mut vec: InplaceVec<2, String> = InplaceVec::new();
///     vec.push("Hello".to_string());
///     println!("{:?}", vec.as_ptr());
///     vec.into_iter()
/// };
/// let s = iter.next().unwrap();
/// ```
///
/// 上面的代码是完全不会报错的，但是此时vec的分配空间已经被释放了，
/// 也就是说iter中保存的指针已经悬空了，此时调用`iter.next()`就是未
/// 定义行为。
///
/// 我们也不能够直接保存一个`InplaceVec`或`[MaybeUninit<T>; N]`的实
/// 例，再配合`RawValIter`，因为这会导致当我们尝试移动`IntoIter`时，
/// `RawValIter`的指针仍然指向移动前的位置。
///
/// 因此，我们不妨直接抛弃`RawValIter`的指针的逻辑，直接使用索引，在
/// 每次迭代时都计算指针位置。这会有一定的性能损失，但是可以保证行为
/// 的正确。
///
/// 这时，如下的代码就可以正确运行了：
/// ```rust
/// use rust_practice::collection::inplace_vec::InplaceVec;
///
/// let mut iter = {
///     let mut vec: InplaceVec<2, String> = InplaceVec::new();
///     vec.push("Hello".to_string());
///     vec.into_iter()
/// };
/// let s = iter.next().unwrap();
/// println!("{s}");
/// ```
pub struct IntoIter<const N: usize, T> {
    buf: [MaybeUninit<T>; N],
    begin: usize,
    end: usize,
}

impl<const N: usize, T> Iterator for IntoIter<N, T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.begin == self.end {
            None
        } else {
            unsafe {
                let value = self.buf[self.begin].assume_init_read();
                self.begin += 1;
                Some(value)
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<const N: usize, T> DoubleEndedIterator for IntoIter<N, T> {
    fn next_back(&mut self) -> Option<T> {
        if self.begin == self.end {
            None
        } else {
            unsafe {
                self.end -= 1;
                let value = self.buf[self.end].assume_init_read();
                Some(value)
            }
        }
    }
}

impl<const N: usize, T> ExactSizeIterator for IntoIter<N, T> {
    fn len(&self) -> usize {
        self.end - self.begin
    }
}

impl<const N: usize, T> FusedIterator for IntoIter<N, T> {}

impl<const N: usize, T> Drop for IntoIter<N, T> {
    fn drop(&mut self) {
        unsafe {
            let drop_array: *mut [T] =
                slice::from_raw_parts_mut(self.buf.as_mut_ptr().add(self.begin).cast(), self.len());
            std::ptr::drop_in_place(drop_array);
        }
    }
}

impl<const N: usize, T> IntoIterator for InplaceVec<N, T> {
    type Item = T;
    type IntoIter = IntoIter<N, T>;
    fn into_iter(self) -> Self::IntoIter {
        unsafe {
            let buf = ptr::read(&self.buf);
            let begin = 0;
            let end = self.len;
            mem::forget(self);
            IntoIter { buf, begin, end }
        }
    }
}

impl<'a, const N: usize, T> IntoIterator for &'a InplaceVec<N, T> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, const N: usize, T> IntoIterator for &'a mut InplaceVec<N, T> {
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
