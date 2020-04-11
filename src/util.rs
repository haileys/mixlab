use std::num::NonZeroUsize;

pub struct Sequence(usize);

impl Sequence {
    pub fn new() -> Self {
        Sequence(0)
    }

    pub fn next(&mut self) -> NonZeroUsize {
        self.0 += 1;
        NonZeroUsize::new(self.0).unwrap()
    }
}

pub fn zero(slice: &mut [f32]) {
    for sample in slice.iter_mut() {
        *sample = 0.0;
    }
}
