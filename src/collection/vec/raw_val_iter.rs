use std::mem;
use std::ptr::{self, NonNull};

/// 源自The Rustonomicon
///
/// [`RawValIter`]是指向待迭代区域头部和超尾的指针的抽象，用
/// 于提供按值迭代功能，其名义上拥有被迭代内容的所有权，但实
/// 际并不拥有，使用时需要保证缓冲区的生命周期长于该类型。
///
/// 见[`IntoIter`]中的描述。
pub(super) struct RawValIter<T> {
    start: *const T,
    end: *const T,
}

impl<T> RawValIter<T> {
    /// 源自The Rustonomicon
    ///
    /// 该构造是不安全的，因为`&[T]`不会绑定声明周期，因此必须保证
    /// `&[T]`的生命周期不长于分配的内存空间。由于仅用于private实现，
    /// 所以没有问题。
    ///
    /// ## 关于特别讨论`slice.is_empty`的问题
    ///
    /// 此处我们需要特别注意，由于`size`为0，我们不能保证原[`MyVec`]
    /// 是否是`cap`为0的，在此情况下，其`ptr`可能为`NonNull::dangling`，
    /// 换而言之，它是没有`Provenance`的，根据标准库文档所述，这种
    /// 指针在访问数据（ZST除外）、偏移非0长度时是无效的。
    ///
    /// 引用如下：
    ///
    /// > Create a pointer without provenance from just an address (see without_provenance).
    /// > Such a pointer cannot be used for memory accesses (except for zero-sized accesses).
    /// > This can still be useful for sentinel values like null or to represent
    /// > a tagged pointer that will never be dereferenceable. In general, it is
    /// > always sound for an integer to pretend to be a pointer “for fun” as long as
    /// > you don’t use operations on it which require it to be valid (non-zero-sized offset,
    /// > read, write, etc).
    ///
    /// 也就是说，根据上述说明，对于一个没有`Provenance`的指针进行
    /// 长度为0的偏移是合法的。
    ///
    /// 在对于`pointer::add`的文档中提到：
    ///
    /// > If the computed offset is non-zero, then self must be derived from a pointer to
    /// > some allocation, and the entire memory range between self and the result must be
    /// > in bounds of that allocation. In particular, this range must not “wrap around”
    /// > the edge of the address space.
    ///
    /// 上面的内容说明了如果是`add(0)`或是`offset(0)`（在此不给出引
    /// 用了），其实都是合法的。
    ///
    /// 那么为什么要有一个这么一个分支呢？
    ///
    /// [`rust-lang/rust#65108`](https://github.com/rust-lang/rust/issues/65108)
    /// 提到了这个问题，`RalfJung`的[回答](https://github.com/rust-lang/rust/issues/65108#issuecomment-540076878)
    /// 如下：
    ///
    /// > The problem is that we don't really know what LLVM's rules are for
    /// > `getelementptr inbounds` with a 0 offset. Also see #54857 of which
    /// > this seems to be a duplicate, ...
    ///
    /// 具体可以看`RalfJung`给出的更多网页，总结来说是这样的：
    ///
    /// - 这个行为其实是未定义的，但对于一个非ZST的`integer pointer`
    ///   （即直接使用`as`将[`usize`]转换为pointer）来说，本来就是不
    ///   允许访问任何内容的，而因此`RalfJung`认为对`integer pointer`
    ///   来说，偏移0是合法的。但此处必须强调，`integer pointer`由于
    ///   其本身的性质，访问内存是UB，但对于一个正常的指针来说，他认
    ///   为位移为0也是不合法的。
    /// - 目前`Miri`的实现认为对`integer pointer`位移为0是合法的，不
    ///   会有报错。
    ///
    /// 因此，这其实是一个灰色地带，我们无法断言可能会UB，也无法断言
    /// 这不可能UB。目前无论是文档还是官方的工具，都认为这其实是合法
    /// 的。
    ///
    /// 此处，The Rustonomicon保守地增加了一个新的分支。
    pub unsafe fn new(slice: &[T]) -> Self {
        RawValIter {
            start: slice.as_ptr(),
            end: if mem::size_of::<T>() == 0 {
                ((slice.as_ptr() as usize) + slice.len()) as *const _
            } else if slice.is_empty() {
                // 关于为什么有这个分支的问题，见[`RawValIter::new`]的文档
                slice.as_ptr()
            } else {
                unsafe { slice.as_ptr().add(slice.len()) }
            },
        }
    }
}

impl<T> Iterator for RawValIter<T> {
    type Item = T;

    /// 源自The Rustonomicon
    ///
    /// next的行为如下：
    ///
    /// 1. 获取start指向的内存空间元素的所有权
    /// 2. 向后移动，将刚才读取的位置作为逻辑上未初始化的空间
    fn next(&mut self) -> Option<T> {
        if self.start == self.end {
            None
        } else {
            unsafe {
                if mem::size_of::<T>() == 0 {
                    self.start = (self.start as usize + 1) as *const _;
                    // 我们应当始终保证调用[`ptr::read`]的裸指针是对齐的，即使
                    // 对于ZST来说，`ptr::read`什么也不做。在此处，我们不能保证
                    // `self.start`是对齐的，因此我们选择传入[`NonNull::dangling`]。
                    Some(ptr::read(NonNull::<T>::dangling().as_ptr()))
                } else {
                    let old_ptr = self.start;
                    self.start = self.start.offset(1);
                    // 获取元素所有权，并将其作为逻辑上未初始化空间
                    Some(ptr::read(old_ptr))
                }
            }
        }
    }

    /// 源自The Rustonomicon
    ///
    /// 该迭代器属于[`ExactSizeIterator`]，因此需要重新实现`size_hint`，
    /// 且上界等于下界等于[`ExactSizeIterator::len`]。
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = <Self as ExactSizeIterator>::len(self);
        (len, Some(len))
    }
}

impl<T> DoubleEndedIterator for RawValIter<T> {
    /// 源自The Rustonomicon
    ///
    /// next_back的行为如下：
    /// 1. end指针向前移动
    /// 2. 获取end指向的内存空间元素的所有权，并将该位置作为逻辑上
    ///    未初始化的空间
    fn next_back(&mut self) -> Option<T> {
        if self.start == self.end {
            None
        } else {
            unsafe {
                if mem::size_of::<T>() == 0 {
                    self.end = (self.end as usize - 1) as *const _;
                    Some(ptr::read(NonNull::<T>::dangling().as_ptr()))
                } else {
                    self.end = self.end.offset(-1);
                    Some(ptr::read(self.end))
                }
            }
        }
    }
}

/// 能够确切知道返回元素个数的迭代器
///
/// 理论上来说[`ExactSizeIterator::len`]和[`ExactSizeIterator::is_empty`]
/// 是默认实现的，因此不需要手动实现。但手动实现`len`会更加
/// 高效，而且[`Iterator::size_hint`]也可以利用该函数。
impl<T> ExactSizeIterator for RawValIter<T> {
    fn len(&self) -> usize {
        let elem_size = mem::size_of::<T>();
        (self.end as usize - self.start as usize) / if elem_size == 0 { 1 } else { elem_size }
    }
}
