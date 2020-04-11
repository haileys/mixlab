#![feature(proc_macro_hygiene)]

use fieldwise_derive::{Fieldwise, path};

#[derive(Fieldwise, Debug)]
pub struct Foo {
    one: usize,
    two: Bar,
}

#[derive(Fieldwise, Debug)]
pub struct Bar(usize);

fn main() {
    let mut foo = Foo { one: 1, two: Bar(0) };

    let lens = path!(crate::Foo.two.0);

    {
        use fieldwise::Path;
        *lens.access_mut(&mut foo).unwrap() = 456;
        println!("{:?}", lens.access(&foo));
    }

    println!("foo: {:?}", foo);
}

