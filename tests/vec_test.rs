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
    let vec_units = Vec::<()>::with_capacity(10);
    assert_eq!(vec_units.capacity(), usize::MAX);
}

#[test]
fn vec_from_iter() {
    let five_fives = std::iter::repeat_n(5, 5);

    let v: MyVec<i32> = five_fives.collect();

    assert_eq!(v, my_vec![5, 5, 5, 5, 5]);
}
