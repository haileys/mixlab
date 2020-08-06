use std::convert::TryInto;
use std::ptr;
use std::slice;

use ffmpeg_dev::sys as ff;

use crate::ffmpeg::AvError;
use crate::ffmpeg::ioctx::{IoReader, AvIoReader};

pub struct InputContainer<R: IoReader> {
    ctx: RawContext,
    // must be held alive as it is referenced by AVFormatContext
    // must not be used from rust
    _io: AvIoReader<R>,
}

impl<R: IoReader> InputContainer<R> {
    pub fn open(mut io: AvIoReader<R>) -> Result<Self, AvError> {
        let mut ctx = RawContext::alloc();

        let rc = unsafe {
            (*ctx.ptr).pb = io.as_mut_ptr();

            ff::avformat_open_input(
                &mut ctx.ptr as *mut *mut _,
                ptr::null(), // url
                ptr::null_mut(), // fmt
                ptr::null_mut(), // options
            )
        };

        if rc != 0 {
            return Err(AvError(rc));
        }

        Ok(InputContainer {
            ctx,
            _io: io,
        })
    }

    fn as_underlying(&self) -> &ff::AVFormatContext {
        unsafe { &*(self.ctx.ptr as *const _) }
    }

    pub fn streams(&self) -> &[InputStream] {
        let underlying = self.as_underlying();
        let nb_streams = underlying.nb_streams.try_into()
            .expect("nb_streams as usize");

        unsafe {
            slice::from_raw_parts(underlying.streams as *const *mut _, nb_streams)
        }
    }
}

impl<R: IoReader> Drop for InputContainer<R> {
    fn drop(&mut self) {
        unsafe {
            ff::avformat_close_input(&mut self.ctx.ptr as *mut *mut _);
        }
    }
}

#[repr(transparent)]
pub struct InputStream {
    ptr: *mut ff::AVStream,
}

impl InputStream {
    pub fn id(&self) -> i32 {
        self.as_underlying().id as i32
    }

    fn as_underlying(&self) -> &ff::AVStream {
        unsafe { &*(self.ptr as *const _) }
    }
}

pub struct RawContext {
    ptr: *mut ff::AVFormatContext,
}

impl RawContext {
    pub fn alloc() -> Self {
        unsafe {
            let ptr = ff::avformat_alloc_context();

            if ptr == ptr::null_mut() {
                panic!("avformat_alloc_context: could not allocate");
            }

            RawContext { ptr }
        }
    }
}

impl Drop for RawContext {
    fn drop(&mut self) {
        unsafe {
            // may be freed by avformat_close_input, in which case do not
            // attempt to free again
            if self.ptr != ptr::null_mut() {
                ff::avformat_free_context(self.ptr);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::InputContainer;
    use crate::ffmpeg::ioctx::{IoReader, AvIoReader};
    use std::io::{self, Read, Seek, SeekFrom};
    use std::fs::File;

    impl IoReader for File {
        type Error = io::Error;
        const BUFFER_SIZE: usize = 4096;

        fn read(&mut self, out: &mut [u8]) -> Result<usize, Self::Error> {
            let bytes = Read::read(self, out)?;
            eprintln!("AvIoReader read {} bytes", bytes);
            Ok(bytes)
        }

        fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
            eprintln!("AvIoReader seeked to {:?}", pos);
            Seek::seek(self, pos)
        }

        fn size(&mut self) -> Result<u64, Self::Error> {
            let cur_pos = Seek::seek(self, SeekFrom::Current(0))?;
            let len = Seek::seek(self, SeekFrom::End(0))?;
            Seek::seek(self, SeekFrom::Start(cur_pos))?;
            Ok(len)
        }
    }

    #[test]
    fn basic_probe() {
        let file = File::open("/Users/charlie/Movies/Real Scenes - Melbourne _ Resident Advisor-cs1Iw-r0YI8.mp4").unwrap();
        let avio = AvIoReader::new(file);
        let fmt = InputContainer::open(avio).unwrap();

        println!("streams:");

        for stream in fmt.streams() {
            println!("  - {:?}", stream.id());
        }
    }
}
