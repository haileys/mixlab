use std::io;
use std::num::NonZeroUsize;

use num_rational::Rational64;
use futures::executor::block_on;
use tokio::io::AsyncRead;

#[derive(Debug)]
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

pub struct SyncRead<T>(pub T);

impl<T: AsyncRead + Unpin> io::Read for SyncRead<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        use tokio::io::AsyncReadExt;
        block_on(self.0.read(buf))
    }
}

#[allow(unused)]
pub fn decimal(ratio: Rational64) -> String {
    let micros = (ratio * 1_000_000).to_integer();
    format!("{:.3}", micros as f64 / 1_000_000.0)
}
