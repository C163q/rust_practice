use std::alloc::{self, Layout};
use std::mem;
use std::ptr::NonNull;

/// 源自The Rustonomicon
///
/// ## 类型介绍
///
/// [`MyRawVec`]是`ptr`和`cap`元组的抽象，其目的是合并
/// [`MyVec`]和[`IntoIter`]当中重复的逻辑。
///
/// `MyRawVec`用于管理内存的分配、释放和容量增长逻辑。
///
/// 其包含[`NonNull<T>`]类型的`ptr`（表示指向分配的内存
/// 空间）和[`usize`]类型的`cap`（表示最多可以容纳的元素
/// 个数）
///
/// ## 成员类型选择
///
/// `ptr`不应当使用`*mut T`，其原因是在此情况下，`MyRawVec<T>`
/// 在`T`上是不变的(invariant)。这就导致`MyVec<T>`在`T`上
/// 是不变的。也就是说，一个`&MyVec<&'static str>`不能传
/// 给需要`&MyVec<&'a str>`的地方。
///
/// `NonNull<T>`在`T`上是协变的(covariant)，因此`MyVec<T>`
/// 在`T`上是协变的。此外，`NonNull`保证其指针永远不为空，
/// 从而允许空指针优化。
///
/// `NonNull`的是`*const T`的包装，因此它是协变的。一个指
/// 向可变变量的const指针转换为mut指针不是未定义行为，因此
/// 通过`NonNull`获取`*mut T`是安全的。
#[derive(Debug)]
pub(super) struct MyRawVec<T> {
    ptr: NonNull<T>,
    cap: usize,
}

/// 源自The Rustonomicon
///
/// [`NonNull`]本身没有实现[`Send`]和[`Sync`] trait，因此
/// 需要手动去实现。
///
/// 一个类型是`Send`的，必须保证该类型可以被安全的发送到另
/// 外一个线程。如果[`MyVec`]中所拥有的元素是`Send`的，则
/// 整个`MyVec`当然就可以发送到另外一个线程。
///
/// 一个类型是`Sync`的，必须保证该类型可以安全的在线程之间
/// 共享，即`T`是`Sync`当且仅当`&T`是`Send`。如果`MyVec`中
/// 所拥有的元素是`Sync`的，则整个`MyVec`当然可以安全的在线
/// 程之间共享。
unsafe impl<T: Send> Send for MyRawVec<T> {}
unsafe impl<T: Sync> Sync for MyRawVec<T> {}

impl<T> MyRawVec<T> {
    #[inline]
    pub fn ptr(&self) -> NonNull<T> {
        self.ptr
    }

    #[inline]
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// 源自The Rustonomicon
    ///
    /// 内存分配器（global allocator）不允许我们申请0字节的空间，
    /// 这会导致未定义行为。
    ///
    /// 以下摘自C语言`malloc`函数的参考：
    ///
    /// > If size is zero, the behavior of malloc is implementation-defined.
    ///
    /// `realloc`的参考如下：
    ///
    /// > if `new_size` is zero, the behavior is undefined. (since C23)
    ///
    /// 由于[`NonNull`]不允许存放空指针，因此可以存放
    /// [`NonNull::dangling`]，这其实就是存入了[`mem::align_of`]。
    /// 我们应当始终保证`ptr`是指向对齐的内存的，即使我们不去访问
    /// 它。原因是，可能会有外部的代码去获取该指针，并使用
    /// [`ptr::read`]尝试去读取这块内存，而`ptr::read`要求必须对
    /// 齐。虽然读取长度为0字节的内存时（也就是此处讨论的情况），
    /// `ptr::read`其实什么都不做，但还是要避免这种情况。
    ///
    /// 这一要求可以见标准库[`ptr`](https://doc.rust-lang.org/stable/std/ptr/index.html#alignment)文档：
    ///
    /// > When a function requires proper alignment, it does so even if
    /// > the access has size 0, i.e., even if memory is not actually touched.
    ///
    /// 关于判断`cap == 0`以避免访问悬挂指针的问题：由于在一般情
    /// 况下，都会判断`len > cap`或者`len > 0`，这隐含了`cap > 0`，
    /// 因此绝大多数情况下都不需要判断。
    ///
    /// 对于ZST来说，不存在所谓的内存溢出或者offset有符号数的问题
    /// （因为任何指针偏移操作都被认为是无操作），所以可以将其容
    /// 量设置为`usize::MAX`。但此处需要考虑到在[`RawValIter`]中，
    /// 我们对ZST进行了特殊讨论，`start`是`NonNull::dangling`，而
    /// `end as usize - start as usize`是元素数量，考录到如果`size`
    /// 为`usize::MAX`，则end必然会溢出，考虑到这个问题，我们选择
    /// 将其设置为[`isize::MAX`]。
    ///
    /// 相关问题见[rust-lang/nomicon#433](https://github.com/rust-lang/nomicon/issues/433)
    pub fn new() -> Self {
        // 下面的分支可以在编译期确定。
        let cap = if mem::size_of::<T>() == 0 {
            isize::MAX as usize
        } else {
            0
        };

        // `NonNull::dangling()`既可以表示未分配空间，也可以表示分配大小为0的空间
        MyRawVec {
            ptr: NonNull::dangling(),
            cap,
        }
    }

    /// 源自The Rustonomicon
    ///
    /// 关于内存分配方面，存在两种情况，一种是在正常使用的情况下，
    /// 系统out-of-memory(OOM)了，这时候应当使用
    /// [`alloc::handle_alloc_error`]来处理这种异常，而不是直接
    /// [`panic!`]，原因是[`panic!`]会导致unwind，而在此过程中仍
    /// 然会试图申请内存。而`alloc::handle_alloc_error`则会以系
    /// 统特定的方式终止程序。
    ///
    /// 不过，在这种情况下，现在的操作系统一般都会以某种方式直接
    /// 杀死程序，理论上来说，可以不用关心。
    ///
    /// ## 关于内存分配上限以及内存访问方面
    ///
    /// 根据Rustonomicon所述，`LLVM`的`GetElementPtr`(GEP)允许高
    /// 度的优化。例如，如果存在两个指针，其不指向同一块内存空间，
    /// 那么编译器就可以让对两个指针的操作并行发生。
    ///
    /// 因此如果存在两个指针，这两个指针源自不同的分配内存，那么
    /// 任何对指针的偏移操作都会被认为源自不同的分配内存，使得编
    /// 译器执行更为激进的优化。这种偏移操作被称为`inbounds`。
    ///
    /// 从上面也可以看出，一旦指针的位移操作导致其落入分配空间之
    /// 外（不包括其尾后内存空间），行为都是未定义的。此外，C和
    /// C++都没有明确规定指针是否允许回绕，就网上的资料来看，应该
    /// 是不允许的。见[reddit上的一个帖子](https://www.reddit.com/r/C_Programming/comments/1czemhj/undefined_behavior_in_pointer_arithmetic_with/)
    ///
    /// GEP指令接受的是有符号整数类型，所以[`pointer::offset`]也
    /// 是如此（参数为[`isize`]类型），由于索引时传入的[`usize`]，
    /// 这就可能会超过[`isize::MAX`]导致溢出。在这种情况下，必然
    /// 会传入一个负值，在此情况下，指针可能会指向一个分配空间外
    /// 的地址，这必然会导致未定义行为，因为GEP的inbounds优化的前
    /// 提条件是位移前后必须保证其位于同一分配空间或其超尾处。当
    /// 然也有可能会绕一圈然后回到同一分配空间中（也就是依赖整型
    /// 的溢出），但这个行为也是没有定义的，原因是不同硬件架构不
    /// 同，并非所有硬件都是从0到[`usize::MAX`]是连续的。
    ///
    /// 补充：针对上述`isize`溢出，导致变为负数的问题，在标准库
    /// [`ptr`](https://doc.rust-lang.org/stable/std/ptr/index.html#provenance)中有以下内容：
    ///
    /// > It is undefined behavior to offset a pointer across a memory range
    /// > that is not contained in the allocation it is derived from, or to
    /// > `offset_from` two pointers not derived from the same allocation.
    ///
    /// 也就是说，即使真正发生了回绕，但是指针“穿过”了`Provenance`
    /// 限定的`Spatial`以外区域，这就足以导致未定义行为了。
    ///
    /// 对于`pointer::offset`，rust在Debug下会在运行时检查是否会
    /// 溢出，并且执行内建的指针移位操作，通常会使用GEP inbounds带
    /// 来的优化。如果可能会导致回绕，就需要使用[`pointer::wrapping_offset`]，
    /// 它在底层其实就是执行补码回绕运算，行为是确定的，但不会执
    /// 行上述的优化。
    ///
    /// 由于[`pointer::offset`]实际上是偏移了`size_of::<T>() * count`
    /// 字节，因此即使`count < isize::MAX`但`size_of::<T>() * count > isize::MAX`，
    /// 依然不属于溢出（只要遵循[`pointer::offset`]的要求）。
    ///
    /// 所以，理论上来说，只需要防止大小为1字节的类型分配超过
    /// `isize::MAX`个元素即可，因为如果2字节的类型分配这么多的话，
    /// 首先就会爆内存了，但是为了避免使用[`mem::transmute`]将后
    /// 者转换为前者而导致的问题，因此必须保证分配的内存的大小不
    /// 能超过`isize::MAX`字节。
    ///
    /// 对于64位平台来说，这完全是OK的，因为一般不会有这么大的内
    /// 存。但对于32位平台来说就有可能会分配这么大的空间。此处我
    /// 不做考虑。
    ///
    /// ## 关于ZST的问题
    ///
    /// ZST占据0字节的空间，因此任何对ZST的指针的解引用都不会访问
    /// 任何内存数据。对ZST的指针的任何偏移都不会进行任何操作，所
    /// 以它是满足GEP的要求的。
    ///
    /// 一般来说，我们会认为在0x01处存在一个可以存放无限多个ZST元
    /// 素的空间，该空间不能为0x00，因为不能使用该地址，此外，整个
    /// 内存的第一页（一般是前4KB空间）一般是受到保护不会被分配的。
    pub fn grow(&mut self) {
        // 由于我们已经将ZST的容量设置为isize::MAX了，所以如果ZST
        // 执行了这个函数必然表示其容量溢出了。
        assert!(mem::size_of::<T>() != 0, "capacity overflow");

        let (new_cap, new_layout) = if self.cap == 0 {
            (1, Layout::array::<T>(1).unwrap())
        } else {
            // 由于此处self.cap <= isize::MAX的，所以下面的表达式不会溢出
            let new_cap = 2 * self.cap;

            // `Layout::array`会检查字节数是小于等于isize::MAX的，但由于
            // 这正是我们希望检查的，我们希望在字节数超过isize::MAX时直接
            // panic。
            let new_layout = Layout::array::<T>(new_cap).expect("Allocation too large");
            (new_cap, new_layout)
        };

        // SAFETY:
        // 注意，使用realloc申请0字节空间是未定义行为，但在此处，我们
        // 保证其大小至少为1字节。ZST类型的`cap`永远都是`isize::MAX`，
        // 所以应该不会执行此处的代码。
        let new_ptr = unsafe { self.try_alloc_nonzeroed(new_layout) };

        self.ptr = Self::handle_alloc_err(new_ptr as *mut T, new_layout);
        self.cap = new_cap;
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let mut ret = Self::new();
        if mem::size_of::<T>() != 0 && capacity > 0 {
            let layout = Layout::array::<T>(capacity).expect("Allocation too large");
            let ptr = unsafe { ret.try_alloc_new(layout) };

            ret.ptr = Self::handle_alloc_err(ptr as *mut T, layout);
            ret.cap = capacity;
        }
        ret
    }

    /// ## safety
    /// 此处必须保证exact_cap不会超过`isize::MAX`，即使是ZST！
    pub unsafe fn reserve_exact(&mut self, exact_cap: usize) {
        if exact_cap <= self.cap {
            return;
        }

        let new_layout = Layout::array::<T>(exact_cap).expect("Allocation too large");
        let new_ptr = self.try_alloc(new_layout);

        self.ptr = Self::handle_alloc_err(new_ptr as *mut T, new_layout);
        self.cap = exact_cap;
    }

    /// 如果分配失败了，`new_ptr`会是空指针，对应产生None，此处使用
    /// `alloc::handle_alloc_error`终止程序。
    #[inline]
    pub fn handle_alloc_err(ptr: *mut T, new_layout: Layout) -> NonNull<T> {
        match NonNull::new(ptr) {
            Some(p) => p,
            None => alloc::handle_alloc_error(new_layout),
        }
    }

    #[inline]
    pub fn try_alloc(&mut self, new_layout: Layout) -> *mut u8 {
        if new_layout.size() == 0 {
            unsafe { self.try_alloc_zeroed() }
        } else {
            unsafe { self.try_alloc_nonzeroed(new_layout) }
        }
    }

    #[inline]
    unsafe fn try_alloc_zeroed(&mut self) -> *mut u8 {
        if self.cap != 0 {
            let old_layout = Layout::array::<T>(self.cap).unwrap();
            let old_ptr = self.ptr.as_ptr() as *mut u8;
            unsafe {
                alloc::dealloc(old_ptr, old_layout);
            }
        }
        NonNull::dangling().as_ptr()
    }

    #[inline]
    unsafe fn try_alloc_nonzeroed(&mut self, new_layout: Layout) -> *mut u8 {
        if self.cap == 0 {
            unsafe { self.try_alloc_new(new_layout) }
        } else {
            unsafe { self.try_realloc(new_layout) }
        }
    }

    #[inline]
    unsafe fn try_alloc_new(&mut self, new_layout: Layout) -> *mut u8 {
        unsafe { alloc::alloc(new_layout) }
    }

    /// ## safety
    ///
    /// - `new_layout.size`应当保证不为0
    /// - 类型T不应当是ZST
    #[inline]
    unsafe fn try_realloc(&mut self, new_layout: Layout) -> *mut u8 {
        let old_layout = Layout::array::<T>(self.cap).unwrap();
        let old_ptr = self.ptr.as_ptr() as *mut u8;
        unsafe { alloc::realloc(old_ptr, old_layout, new_layout.size()) }
    }
}

impl<T> Drop for MyRawVec<T> {
    /// 源自The Rustonomicon
    ///
    /// 此处我们实现[`MyRawVec::drop`]，由于[`MyRawVec`]仅负责
    /// 管理内存分配，因此我们不应当干预其中的元素。相反，我们
    /// 认为其中的元素都被合理地drop了。
    ///
    /// 我们不应当尝试释放未分配的内存，而对于ZST和`cap == 0`的
    /// 情况下，内存是未分配的，此时不应当调用[`alloc::dealloc`]。
    fn drop(&mut self) {
        let elem_size = mem::size_of::<T>();

        if self.cap != 0 && elem_size != 0 {
            unsafe {
                alloc::dealloc(
                    self.ptr.as_ptr() as *mut u8,
                    Layout::array::<T>(self.cap).unwrap(),
                );
            }
        }
    }
}
