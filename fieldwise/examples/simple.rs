use fieldwise::{Path, Compose};
use fieldwise_derive::Fieldwise;

#[derive(Fieldwise)]
pub struct Foo {
    one: usize,
    two: Bar,
}

#[derive(Fieldwise)]
pub struct Bar {
    three: usize,
}

fn main() {
    let mut foo = Foo { one: 1, two: Bar { three: 3 } };

    let lens = Compose(Foo__two(Foo__), Bar__three(Bar__));

    *lens.access_mut(&mut foo).unwrap() = 456;
    println!("{:?}", lens.access(&foo));
}
