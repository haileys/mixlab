use std::any::Any;
use std::convert::{TryFrom, TryInto};
use std::io::SeekFrom;
use std::marker::PhantomData;
use std::mem;
use std::os::raw::{c_void, c_int};
use std::panic::{self, UnwindSafe, AssertUnwindSafe};
use std::ptr;
use std::slice;

use ffmpeg_dev::sys as ff;

use crate::ffmpeg::{AvError, MIXLAB_IOCTX_ERROR, MIXLAB_IOCTX_PANIC};

pub trait IoReader {
    type Error;
    const BUFFER_SIZE: usize;

    fn read(&mut self, out: &mut [u8]) -> Result<usize, Self::Error>;
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error>;
    fn size(&mut self) -> Result<u64, Self::Error>;
}

pub struct AvIoReader<R: IoReader> {
    ctx: ReaderContext,
    reader: *mut ReaderState<R>,
    _phantom: PhantomData<R>,
}

impl<R: IoReader> AvIoReader<R> {
    #[allow(unused)]
    pub fn new(reader: R) -> Self {
        let reader = Box::into_raw(Box::new(ReaderState {
            error: None,
            reader,
        }));

        let ctx = ReaderContext::alloc(
            R::BUFFER_SIZE,
            reader as *mut c_void,
            read::<R>,
            seek::<R>,
        );

        return AvIoReader {
            ctx,
            reader,
            _phantom: PhantomData,
        };

        unsafe extern "C" fn read<R: IoReader>(opaque: *mut c_void, buf: *mut u8, buf_size: c_int) -> c_int {
            let state = &mut *(opaque as *mut ReaderState<R>);

            state.run_callback(|reader| {
                let buf = slice::from_raw_parts_mut(buf, usize::try_from(buf_size).expect("read callback: buf_size as usize"));

                reader.read(buf).map(|bytes| {
                    // this should never overflow, because buf_size is a c_int
                    // and the largest this could possibly be is also a c_int
                    bytes as c_int
                })
            })
        }

        unsafe extern "C" fn seek<R: IoReader>(opaque: *mut c_void, pos: i64, whence: c_int) -> i64 {
            let state = &mut *(opaque as *mut ReaderState<R>);

            state.run_callback(|reader| {
                if (whence & ff::AVSEEK_SIZE as i32) != 0 {
                    return Ok(i64::try_from(reader.size()?).expect("size too large"));
                }

                let pos = match whence as u32 {
                    ff::SEEK_SET => SeekFrom::Start(pos.try_into().expect("pos to be >= 0")),
                    ff::SEEK_CUR => SeekFrom::Current(pos),
                    ff::SEEK_END => SeekFrom::End(pos),
                    _ => { return Ok(-i64::from(ff::EINVAL)); }
                };

                reader.seek(pos).map(|pos| i64::try_from(pos).expect("u64 too large"))
            })
        }
    }

    pub fn as_mut_ptr(&mut self) -> *mut ff::AVIOContext {
        self.ctx.ptr
    }

    pub fn check_error(&mut self, rc: c_int) -> Result<(), AvIoError<R>> {
        if rc == 0 {
            Ok(())
        } else {
            match self.take_last_error() {
                Some(ReaderError::Error(e)) => {
                    return Err(AvIoError::Io(e));
                }
                Some(ReaderError::Panic(payload)) => {
                    panic::resume_unwind(payload);
                }
                None => {
                    Err(AvIoError::Av(AvError(rc)))
                }
            }
        }
    }

    fn take_last_error(&mut self) -> Option<ReaderError<R::Error>> {
        unsafe { &mut *self.reader }.error.take()
    }
}

impl<R: IoReader> Drop for AvIoReader<R> {
    fn drop(&mut self) {
        unsafe {
            mem::drop(Box::from_raw(self.reader));
        }
    }
}

#[derive(Debug)]
pub enum AvIoError<R: IoReader> {
    Io(R::Error),
    Av(AvError),
}

struct ReaderState<R: IoReader> {
    error: Option<ReaderError<R::Error>>,
    reader: R,
}

impl<R: IoReader> ReaderState<R> {
    fn run_callback<T: From<i32>>(&mut self, f: impl FnOnce(&mut R) -> Result<T, R::Error> + UnwindSafe) -> T {
        let result = panic::catch_unwind(AssertUnwindSafe(|| f(&mut self.reader)));

        match result {
            Ok(Ok(value)) => {
                value
            }
            Ok(Err(e)) => {
                self.error = Some(ReaderError::Error(e));
                MIXLAB_IOCTX_ERROR.into()
            }
            Err(e) => {
                self.error = Some(ReaderError::Panic(e));
                MIXLAB_IOCTX_PANIC.into()
            }
        }
    }
}

pub enum ReaderError<T> {
    Error(T),
    Panic(Box<dyn Any + Send + 'static>),
}

struct ReaderContext {
    ptr: *mut ff::AVIOContext,
}

impl ReaderContext {
    pub fn alloc(
        buffer_size: usize,
        opaque: *mut c_void,
        read_packet: unsafe extern "C" fn(*mut c_void, *mut u8, c_int) -> c_int,
        seek: unsafe extern "C" fn(*mut c_void, i64, c_int) -> i64,
    ) -> Self {
        let buffer_size_int = c_int::try_from(buffer_size)
            .expect("buffer_size to fit in c_int");

        let buffer = unsafe { ff::av_malloc(buffer_size) };

        if buffer == ptr::null_mut() {
            panic!("av_malloc: could not allocate");
        }

        let ptr = unsafe {
            ff::avio_alloc_context(
                buffer as *mut u8,
                buffer_size_int,
                0, // write flag
                opaque,
                Some(read_packet),
                None, // write packet fn
                Some(seek),
            )
        };

        if ptr == ptr::null_mut() {
            unsafe { ff::av_free(buffer); }
            panic!("avio_alloc_context: could not allocate");
        }

        ReaderContext { ptr }
    }
}

impl Drop for ReaderContext {
    fn drop(&mut self) {
        if self.ptr != ptr::null_mut() {
            unsafe {
                ff::avio_context_free(&mut self.ptr as *mut *mut _);
            }
        }
    }
}
