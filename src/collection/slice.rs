use core::panic;
use std::ops::{Bound, Range, RangeBounds, RangeTo};

/// 由于[`std::slice::range`]到目前`1.90.0`为止，仍然
/// 是不稳定的特性，因此我们在此处自己实现它。
pub fn range<R: RangeBounds<usize>>(range: R, bounds: RangeTo<usize>) -> Range<usize> {
    let (lower, upper) = (range.start_bound(), range.end_bound());

    let left = match lower {
        Bound::Unbounded => 0,
        Bound::Included(&l) => l,
        Bound::Excluded(&l) => l
            .checked_add(1)
            .expect("attempted to index slice from after maximum usize"),
    };

    let right = match upper {
        Bound::Unbounded => bounds.end,
        Bound::Included(&u) => u
            .checked_add(1)
            .expect("attempted to index slice up to maximum usize"),
        Bound::Excluded(&u) => u,
    };

    // 由于我们这里是来自两个`RangeBounds`的，因此就会导致可能会有
    // 左边界大于右边界的情况，这是不允许的！
    if left > right {
        panic!("invaild slice bounds whose left index is larger than right");
    }

    if right > bounds.end {
        panic!("right index is out of bounds");
    }

    left..right
}
