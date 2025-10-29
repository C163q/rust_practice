use rust_practice::collection::inplace_vec::InplaceVec;

fn main() {
    let mut iter = {
        let mut vec: InplaceVec<2, String> = InplaceVec::new();
        vec.push("Hello".to_string());
        println!("{:?}", vec.as_ptr());
        vec.into_iter()
    };
    let s = iter.next().unwrap();
    println!("{s}");

    let mut vec: InplaceVec<5, i32> = InplaceVec::new();
    vec.push(1);
    vec.push(2);
    vec.push(3);
    vec.push(4);
    vec.push(5);
    let d = vec.drain(2..4);
    for i in d {
        println!("Drained: {}", i);
    }
    for i in vec {
        println!("Remaining: {}", i);
    }
}
