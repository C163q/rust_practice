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
}
