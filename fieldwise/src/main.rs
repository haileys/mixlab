pub trait Path {
    type Root: Path;
    type Item;

    fn access<'a>(&self, root: &'a <Self::Root as Path>::Item) -> Option<&'a Self::Item>;
    fn access_mut<'a>(&self, root: &'a mut <Self::Root as Path>::Item) -> Option<&'a mut Self::Item>;
}

pub struct Compose<A, B>(A, B);

impl<A: Path<Item = Ap::Item>, B: Path<Root = Ap>, Ap: Path + 'static> Path for Compose<A, B> {
    type Root = A::Root;
    type Item = B::Item;

    fn access<'a>(&self, root: &'a <Self::Root as Path>::Item) -> Option<&'a Self::Item> {
        self.1.access(self.0.access(root)?)
    }

    fn access_mut<'a>(&self, root: &'a mut <Self::Root as Path>::Item) -> Option<&'a mut Self::Item> {
        self.1.access_mut(self.0.access_mut(root)?)
    }
}

// #[derive(Fieldwise)]
pub struct Foo {
    one: usize,
    two: Bar,
}

// {{{ FIELDWISE GENERATED:

    #[derive(Clone)]
    pub struct Foo__;

    impl Path for Foo__ {
        type Root = Foo__;
        type Item = Foo;

        fn access<'a>(&self, root: &'a <Self::Root as Path>::Item) -> Option<&'a Self::Item> {
            Some(root)
        }

        fn access_mut<'a>(&self, root: &'a mut <Self::Root as Path>::Item) -> Option<&'a mut Self::Item> {
            Some(root)
        }
    }

    #[allow(non_camel_case_types)]
    pub struct Foo__one<B: Path>(B);

    impl<B: Path<Item = Foo>> Path for Foo__one<B> {
        type Root = B::Root;
        type Item = usize;

        fn access<'a>(&self, root: &'a <Self::Root as Path>::Item) -> Option<&'a Self::Item> {
            Some(&self.0.access(root)?.one)
        }

        fn access_mut<'a>(&self, root: &'a mut <Self::Root as Path>::Item) -> Option<&'a mut Self::Item> {
            Some(&mut self.0.access_mut(root)?.one)
        }
    }

    #[allow(non_camel_case_types)]
    pub struct Foo__two<B: Path>(B);

    impl<B: Path<Item = Foo>> Path for Foo__two<B> {
        type Root = B::Root;
        type Item = Bar;

        fn access<'a>(&self, root: &'a <Self::Root as Path>::Item) -> Option<&'a Self::Item> {
            Some(&self.0.access(root)?.two)
        }

        fn access_mut<'a>(&self, root: &'a mut <Self::Root as Path>::Item) -> Option<&'a mut Self::Item> {
            Some(&mut self.0.access_mut(root)?.two)
        }
    }
// END }}}

// #[derive(Fieldwise)]
pub struct Bar {
    three: usize,
}

// {{{FIELDWISE GENERATED:
    #[allow(non_camel_case_types)]
    #[derive(Clone)]
    pub struct Bar__;

    impl Path for Bar__ {
        type Root = Bar__;
        type Item = Bar;

        fn access<'a>(&self, root: &'a <Self::Root as Path>::Item) -> Option<&'a Self::Item> {
            Some(root)
        }

        fn access_mut<'a>(&self, root: &'a mut <Self::Root as Path>::Item) -> Option<&'a mut Self::Item> {
            Some(root)
        }
    }

    #[allow(non_camel_case_types)]
    pub struct Bar__three<B: Path>(B);

    impl<B: Path<Item = Bar>> Path for Bar__three<B> {
        type Root = B::Root;
        type Item = usize;

        fn access<'a>(&self, root: &'a <Self::Root as Path>::Item) -> Option<&'a Self::Item> {
            Some(&self.0.access(root)?.three)
        }

        fn access_mut<'a>(&self, root: &'a mut <Self::Root as Path>::Item) -> Option<&'a mut Self::Item> {
            Some(&mut self.0.access_mut(root)?.three)
        }
    }
// END }}}

fn main() {
    let mut foo = Foo { one: 1, two: Bar { three: 3 } };

    let lens = Compose(Foo__two(Foo__), Bar__three(Bar__));

    *lens.access_mut(&mut foo).unwrap() = 456;
    println!("{:?}", lens.access(&foo));
}
