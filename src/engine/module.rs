use crate::module::ModuleT;

#[derive(Debug)]
pub struct ModuleCtx<T: ModuleT> {
    phantom: std::marker::PhantomData<T>,
}

impl<T: ModuleT> ModuleCtx<T> {
    pub fn new() -> Self {
        ModuleCtx { phantom: std::marker::PhantomData }
    }
}
