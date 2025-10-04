use std::iter;

use rust_practice::{collection::vec::MyVec, my_vec};

#[test]
fn simple_vec_usage_1() {
    let mut vec = MyVec::new();
    vec.push(1);
    vec.push(2);

    assert_eq!(vec.len(), 2);
    assert_eq!(vec[0], 1);

    assert_eq!(vec.pop(), Some(2));
    assert_eq!(vec.len(), 1);

    vec[0] = 7;
    assert_eq!(vec[0], 7);

    vec.extend([1, 2, 3]);

    for x in &vec {
        println!("{x}");
    }
    assert_eq!(vec, [7, 1, 2, 3]);
}

#[test]
fn simple_vec_usage_2() {
    let mut vec1 = my_vec![1, 2, 3];
    vec1.push(4);
    let vec2 = MyVec::from(&[1, 2, 3, 4]);
    assert_eq!(vec1, vec2);
}

#[test]
fn vec_as_mut_ptr() {
    let size = 4;
    let mut x: MyVec<i32> = MyVec::with_capacity(size);
    let x_ptr = x.as_mut_ptr();

    // Initialize elements via raw pointer writes, then set length.
    unsafe {
        for i in 0..size {
            *x_ptr.add(i) = i as i32;
        }
        x.set_len(size);
    }
    assert_eq!(&*x, &[0, 1, 2, 3]);
}

#[test]
fn vec_with_capacity() {
    let mut vec = MyVec::with_capacity(10);

    // The vector contains no items, even though it has capacity for more
    assert_eq!(vec.len(), 0);
    assert!(vec.capacity() >= 10);

    // These are all done without reallocating...
    for i in 0..10 {
        vec.push(i);
    }
    assert_eq!(vec.len(), 10);
    assert!(vec.capacity() >= 10);

    // ...but this may make the vector reallocate
    vec.push(11);
    assert_eq!(vec.len(), 11);
    assert!(vec.capacity() >= 11);

    // A vector of a zero-sized type will always over-allocate, since no
    // allocation is necessary
    let vec_units = MyVec::<()>::with_capacity(10);
    assert_eq!(vec_units.capacity(), isize::MAX as usize);
}

#[test]
fn vec_drain() {
    let mut v = my_vec![1, 2, 3];
    let u: MyVec<_> = v.drain(1..).collect();
    assert_eq!(v, &[1]);
    assert_eq!(u, &[2, 3]);

    // A full range clears the vector, like `clear()` does
    v.drain(..);
    assert_eq!(v, &[]);

    let mut v = my_vec![10, 20, 30];
    let drained: MyVec<_> = v.drain(1..2).collect();
    assert_eq!(drained, [20]);
    assert_eq!(v, [10, 30]);
}

#[test]
fn vec_from_iter() {
    let five_fives = std::iter::repeat_n(5, 5);
    let v: MyVec<i32> = five_fives.collect();
    assert_eq!(v, my_vec![5, 5, 5, 5, 5]);
}

#[test]
fn vec_basic_operate() {
    let mut vec = my_vec!['a', 'b', 'c'];
    vec.insert(1, 'd');
    assert_eq!(vec, ['a', 'd', 'b', 'c']);
    vec.insert(4, 'e');
    assert_eq!(vec, ['a', 'd', 'b', 'c', 'e']);

    let mut v = my_vec!['a', 'b', 'c'];
    assert_eq!(v.remove(1), 'b');
    assert_eq!(v, ['a', 'c']);

    let mut vec = my_vec![1, 2];
    vec.push(3);
    assert_eq!(vec, [1, 2, 3]);

    let mut vec = my_vec![1, 2, 3];
    assert_eq!(vec.pop(), Some(3));
    assert_eq!(vec, [1, 2]);

    vec.clear();
    assert!(vec.is_empty());
}

#[test]
fn vec_len() {
    let a = my_vec![1, 2, 3];
    assert_eq!(a.len(), 3);

    let mut v = MyVec::new();
    assert!(v.is_empty());

    v.push(1);
    assert!(!v.is_empty());

    let v: MyVec<i32> = iter::empty().collect();
    assert!(v.is_empty());
    assert_eq!(v.capacity(), 0);
}

#[test]
fn vec_extend_and_from_slice() {
    let mut vec = my_vec![1];
    vec.extend_from_slice(&[2, 3, 4]);
    assert_eq!(vec, [1, 2, 3, 4]);

    assert_eq!(MyVec::from(&[1, 2, 3][..]), my_vec![1, 2, 3]);
    assert_eq!(MyVec::from(&[1, 2, 3]), my_vec![1, 2, 3]);

    let mut v1 = my_vec![1, 2];
    let v2 = my_vec![3, 4];
    v1.extend(v2.clone());
    assert_eq!(v1, [1, 2, 3, 4]);
    assert_eq!(v2, [3, 4]); // v2 should not be consumed
}

#[test]
fn vec_clone_from() {
    let x = my_vec![5, 6, 7];
    let mut y = my_vec![8, 9, 10];

    y.clone_from(&x);

    // The value is the same
    assert_eq!(x, y);
}

#[test]
fn vec_zst_support() {
    let mut v = MyVec::new();
    assert_eq!(v.len(), 0);
    assert_eq!(v.capacity(), isize::MAX as usize);
    v.push(());
    v.push(());
    assert_eq!(v.len(), 2);
    assert_eq!(v.pop(), Some(()));
    assert_eq!(v.len(), 1);
    v.clear();
    assert!(v.is_empty());
}

#[test]
fn vec_insert_various_positions() {
    let mut v = my_vec![1, 3];
    v.insert(1, 2); // insert in the middle
    assert_eq!(v, [1, 2, 3]);
    v.insert(0, 0); // insert at start
    assert_eq!(v, [0, 1, 2, 3]);
    v.insert(4, 4); // insert at end
    assert_eq!(v, [0, 1, 2, 3, 4]);
}

#[test]
fn vec_remove_various_positions() {
    let mut v = my_vec![10, 20, 30, 40, 50];
    assert_eq!(v.remove(0), 10); // remove from start
    assert_eq!(v, [20, 30, 40, 50]);
    assert_eq!(v.remove(1), 30); // remove from middle
    assert_eq!(v, [20, 40, 50]);
    assert_eq!(v.remove(2), 50); // remove from end
    assert_eq!(v, [20, 40]);
}
