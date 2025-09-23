#[macro_export]
macro_rules! my_vec {
    ( $( $x:expr ),* ) => {
        {
            let mut temp_vec =
                rust_practice::collection::vec::MyVec::new();
            $(
                temp_vec.push($x);
            )*
            temp_vec
        }
    };
}
