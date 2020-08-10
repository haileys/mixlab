use std::convert::TryInto;
use std::ffi::CStr;
use std::mem::MaybeUninit;
use std::ptr;
use std::slice;

use ffmpeg_dev::sys as ff;
use num_rational::Rational64;

use mixlab_util::time::MediaDuration;

use crate::ffmpeg::{AvIoError, AvPacket};
use crate::ffmpeg::ioctx::{IoReader, AvIoReader};

pub struct InputContainer<R: IoReader> {
    ctx: RawContext,
    // must be held alive as it is referenced by AVFormatContext
    // must not be used from rust
    io: AvIoReader<R>,
}

impl<R: IoReader> InputContainer<R> {
    pub fn open(mut io: AvIoReader<R>) -> Result<Self, AvIoError<R>> {
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

        io.check_error(rc)?;

        Ok(InputContainer {
            ctx,
            io,
        })
    }

    fn as_underlying(&self) -> &ff::AVFormatContext {
        unsafe { &*(self.ctx.ptr as *const _) }
    }

    pub fn streams(&self) -> &[InputStream] {
        let underlying = self.as_underlying();

        let ptr = underlying.streams
            as *const *mut ff::AVStream
            as *const InputStream;

        let len = underlying.nb_streams.try_into()
            .expect("nb_streams as usize");

        unsafe { slice::from_raw_parts(ptr, len) }
    }

    pub fn read_packet(&mut self) -> Result<AvPacket, AvIoError<R>> {
        unsafe {
            let mut pkt = MaybeUninit::uninit();

            let rc = ff::av_read_frame(self.ctx.ptr, pkt.as_mut_ptr());
            self.io.check_error(rc)?;

            Ok(AvPacket::new(pkt.assume_init()))
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

    pub fn codec_name(&self) -> Option<&'static str> {
        let codec_id = self.codec_parameters().codec_id;
        let codec = unsafe { ff::avcodec_find_decoder(codec_id) };

        if codec == ptr::null_mut() {
            return None;
        }

        let long_name = unsafe { CStr::from_ptr((*codec).long_name) };
        Some(long_name.to_str().expect("utf8 codec name"))
    }

    pub fn duration(&self) -> MediaDuration {
        MediaDuration::from(self.time_base() * self.as_underlying().duration)
    }

    pub fn time_base(&self) -> Rational64 {
        let underlying = self.as_underlying();
        Rational64::new(underlying.time_base.num.into(), underlying.time_base.den.into())
    }

    fn codec_parameters(&self) -> &ff::AVCodecParameters {
        unsafe { &*self.as_underlying().codecpar }
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

        eprintln!("streams:");

        for stream in fmt.streams() {
            let secs = (stream.duration().as_rational() * 1_000).to_integer() as f64 / 1_000.0;
            eprintln!("  - {:?}: {:?}, {:.3} secs", stream.id(), stream.codec_name(), secs);
        }

        panic!("OK")
    }
}
