use crate::module::ModuleT;
use crate::project::ProjectBaseRef;

#[derive(Debug)]
pub struct ModuleCtx<T: ModuleT> {
    base: ProjectBaseRef,

    // eventually we'll use this type param
    // use phantom now to prevent a giant refactor later
    phantom: std::marker::PhantomData<T>,
}

impl<T: ModuleT> ModuleCtx<T> {
    pub fn new(base: ProjectBaseRef) -> Self {
        ModuleCtx {
            base,
            phantom: std::marker::PhantomData,
        }
    }
}
