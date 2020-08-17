use std::fmt::Debug;
use std::mem;
use std::ptr;

use mixlab_codec::ffmpeg::{AvFrame, PictureSettings};
use mixlab_codec::ffmpeg::media::Video;

#[derive(Debug)]
pub struct ShaderContext {
    dimensions: BufferDimensions,
    output_buffer: wgpu::Buffer,
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_texture_extent: wgpu::Extent3d,
    render_texture: wgpu::Texture,
}

impl ShaderContext {
    pub async fn new(width: usize, height: usize) -> Self {
        let adapter = wgpu::Instance::new(wgpu::BackendBit::PRIMARY)
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                    shader_validation: true,
                },
                None,
            )
            .await
            .unwrap();

        // It is a webgpu requirement that BufferCopyView.layout.bytes_per_row % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT == 0
        // So we calculate padded_bytes_per_row by rounding unpadded_bytes_per_row
        // up to the next multiple of wgpu::COPY_BYTES_PER_ROW_ALIGNMENT.
        // https://en.wikipedia.org/wiki/Data_structure_alignment#Computing_padding
        let dimensions = BufferDimensions::new(width, height);

        // The output buffer lets us retrieve the data as an array
        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (dimensions.padded_bytes_per_row * dimensions.height) as u64,
            usage: wgpu::BufferUsage::MAP_READ | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        let render_texture_extent = wgpu::Extent3d {
            width: dimensions.width as u32,
            height: dimensions.height as u32,
            depth: 1,
        };

        // The render pipeline renders data into this texture
        let render_texture = device.create_texture(&wgpu::TextureDescriptor {
            size: render_texture_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT | wgpu::TextureUsage::COPY_SRC,
            label: None,
        });

        ShaderContext {
            dimensions,
            output_buffer,
            device,
            queue,
            render_texture_extent,
            render_texture,
        }
    }

    pub async fn render(&self) -> AvFrame<Video> {
        // Set the background to be red
        let command_buffer = {
            let mut encoder =
                self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &self.render_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            // Copy the data from the texture to the buffer
            encoder.copy_texture_to_buffer(
                wgpu::TextureCopyView {
                    texture: &self.render_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                },
                wgpu::BufferCopyView {
                    buffer: &self.output_buffer,
                    layout: wgpu::TextureDataLayout {
                        offset: 0,
                        bytes_per_row: self.dimensions.padded_bytes_per_row as u32,
                        rows_per_image: 0,
                    },
                },
                self.render_texture_extent,
            );

            encoder.finish()
        };

        self.queue.submit(Some(command_buffer));

        // Note that we're not calling `.await` here.
        let buffer_slice = self.output_buffer.slice(..);
        let buffer_future = buffer_slice.map_async(wgpu::MapMode::Read);

        // poll the device for completion
        tokio::task::block_in_place(|| self.device.poll(wgpu::Maintain::Wait));

        buffer_future.await.unwrap();

        let padded_buffer = buffer_slice.get_mapped_range();

        let mut frame = AvFrame::blank(&PictureSettings::rgba(self.dimensions.width, self.dimensions.height));

        unsafe {
            let frame_data = frame.frame_data_mut();

            let mut line_ptr = frame_data.data(0); // RGBA pixel format is non-planar
            let line_size = frame_data.stride(0);

            // from the padded_buffer we write just the unpadded bytes into the image
            for chunk in padded_buffer.chunks(self.dimensions.padded_bytes_per_row) {
                let line_data = &chunk[..self.dimensions.unpadded_bytes_per_row];
                ptr::copy(line_data.as_ptr(), line_ptr, self.dimensions.unpadded_bytes_per_row);
                line_ptr = line_ptr.add(line_size);
            }
        }

        // With the current interface, we have to make sure all mapped views are
        // dropped before we unmap the buffer.
        drop(padded_buffer);

        self.output_buffer.unmap();

        frame
    }
}

#[derive(Debug)]
pub struct BufferDimensions {
    width: usize,
    height: usize,
    unpadded_bytes_per_row: usize,
    padded_bytes_per_row: usize,
}

impl BufferDimensions {
    fn new(width: usize, height: usize) -> Self {
        let bytes_per_pixel = mem::size_of::<u32>();
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        let padded_bytes_per_row_padding = (align - unpadded_bytes_per_row % align) % align;
        let padded_bytes_per_row = unpadded_bytes_per_row + padded_bytes_per_row_padding;
        Self {
            width,
            height,
            unpadded_bytes_per_row,
            padded_bytes_per_row,
        }
    }
}
