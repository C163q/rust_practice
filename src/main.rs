use rust_practice::collection::vec::MyVec;

fn main() {
    let mut a = MyVec::new();
    a.push(1);
    a.push(2);
    a.push(3);

    for _ in 0..3 {
        println!("{}", a.pop().unwrap());
    }
}
