use crate::module::ModuleT;

#[derive(Debug)]
pub struct ModuleLink<T: ModuleT> {
    phantom: std::marker::PhantomData<T>,
}

impl<T: ModuleT> ModuleLink<T> {
    pub fn new() -> Self {
        ModuleLink { phantom: std::marker::PhantomData }
    }
}
