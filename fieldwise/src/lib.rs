pub trait Path {
    type Root: Path;
    type Item;

    fn access<'a>(&self, root: &'a <Self::Root as Path>::Item) -> Option<&'a Self::Item>;
    fn access_mut<'a>(&self, root: &'a mut <Self::Root as Path>::Item) -> Option<&'a mut Self::Item>;
}

pub struct Compose<A, B>(pub A, pub B);

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
