use derive_more::From;
use bytes::Buf;
use byteorder::ReadBytesExt;
use std::io::{self, Read};

// Copyright (c) 2018 Takeru Ohta <phjgt308@gmail.com>

// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:

// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

#[derive(Debug)]
pub struct SpsSummary {
    pub profile_idc: u8,
    pub constraint_set_flag: u8,
    pub level_idc: u8,
    pic_width_in_mbs_minus_1: u64,
    pic_height_in_map_units_minus_1: u64,
    frame_mbs_only_flag: u8,
    frame_crop_left_offset: u64,
    frame_crop_right_offset: u64,
    frame_crop_top_offset: u64,
    frame_crop_bottom_offset: u64,
}

#[derive(Debug)]
pub enum SpsReadError {
    EarlyEof,
    InvalidInput,
    UnsupportedProfile { idc: u8 },
}

impl From<Eof> for SpsReadError {
    fn from(_: Eof) -> Self {
        SpsReadError::EarlyEof
    }
}

impl SpsSummary {
    pub fn width(&self) -> usize {
        (self.pic_width_in_mbs_minus_1 as usize + 1) * 16
            - (self.frame_crop_right_offset as usize * 2)
            - (self.frame_crop_left_offset as usize * 2)
    }

    pub fn height(&self) -> usize {
        (2 - self.frame_mbs_only_flag as usize)
            * ((self.pic_height_in_map_units_minus_1 as usize + 1) * 16)
            - (self.frame_crop_bottom_offset as usize * 2)
            - (self.frame_crop_top_offset as usize * 2)
    }

    pub fn read_from(mut reader: impl Buf) -> Result<Self, SpsReadError> {
        if reader.remaining() < 3 {
            return Err(SpsReadError::EarlyEof);
        }

        let profile_idc = reader.get_u8();
        let constraint_set_flag = reader.get_u8();
        let level_idc = reader.get_u8();

        let mut reader = AvcBitReader::new(reader);
        let _seq_parameter_set_id = reader.read_ue()?;

        match profile_idc {
            // does profile have chroma information?
            100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 => {
                let chroma_format_idc = reader.read_ue()?;

                println!("chroma format: {}", chroma_format_idc);

                if chroma_format_idc == 3 {
                    let _separate_colour_plane_flag = reader.read_bit()?;
                }

                let _bit_depth_luma_minus8 = reader.read_ue()?;
                let _bit_depth_chroma_minus8 = reader.read_ue()?;
                let _qpprime_y_zero_transform_bypass_flag = reader.read_bit()?;

                let seq_scaling_matrix_present_flag = reader.read_bit()?;

                if seq_scaling_matrix_present_flag != 0 {
                    let scaling_list_count =
                        if chroma_format_idc == 3 {
                            12
                        } else {
                            8
                        };

                    for i in 0..scaling_list_count {
                        let seq_scaling_list_present_flags = reader.read_bit()?;

                        if seq_scaling_list_present_flags != 0 {
                            let mut last_scale = 8;
                            let mut next_scale = 8;
                            let size_of_scaling_list =
                                if i < 6 {
                                    16
                                } else {
                                    64
                                };

                            for j in 0..size_of_scaling_list {
                                if next_scale != 0 {
                                    let delta_scale = reader.read_ie()?;

                                    next_scale = (last_scale + delta_scale + 256) % 256;
                                }
                                if next_scale != 0 {
                                    last_scale = next_scale;
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        let _log2_max_frame_num_minus4 = reader.read_ue()?;
        let pic_order_cnt_type = reader.read_ue()?;
        match pic_order_cnt_type {
            0 => {
                let _log2_max_pic_order_cnt_lsb_minus4 = reader.read_ue()?;
            }
            1 => {
                let _delta_pic_order_always_zero_flag = reader.read_bit()?;
                let _offset_for_non_ref_pic = reader.read_ue()?;
                let _ffset_for_top_to_bottom_field = reader.read_ue()?;
                let num_ref_frames_in_pic_order_cnt_cycle = reader.read_ue()?;
                for _ in 0..num_ref_frames_in_pic_order_cnt_cycle {
                    let _offset_for_ref_frame = reader.read_ue()?;
                }
            }
            2 => {}
            _ => return Err(SpsReadError::InvalidInput),
        }
        let _num_ref_frames = reader.read_ue()?;
        let _gaps_in_frame_num_value_allowed_flag = reader.read_bit()?;
        let pic_width_in_mbs_minus_1 = reader.read_ue()?;
        let pic_height_in_map_units_minus_1 = reader.read_ue()?;
        let frame_mbs_only_flag = reader.read_bit()?;
        if frame_mbs_only_flag == 0 {
            let _mb_adaptive_frame_field_flag = reader.read_bit()?;
        }
        let _direct_8x8_inference_flag = reader.read_bit()?;
        let frame_cropping_flag = reader.read_bit()?;
        let (
            frame_crop_left_offset,
            frame_crop_right_offset,
            frame_crop_top_offset,
            frame_crop_bottom_offset,
        ) = if frame_cropping_flag == 1 {
            (
                reader.read_ue()?,
                reader.read_ue()?,
                reader.read_ue()?,
                reader.read_ue()?,
            )
        } else {
            (0, 0, 0, 0)
        };

        Ok(SpsSummary {
            profile_idc,
            constraint_set_flag,
            level_idc,
            pic_width_in_mbs_minus_1,
            pic_height_in_map_units_minus_1,
            frame_mbs_only_flag,
            frame_crop_left_offset,
            frame_crop_right_offset,
            frame_crop_top_offset,
            frame_crop_bottom_offset,
        })
    }
}

struct Eof;

#[derive(Debug)]
struct AvcBitReader<R> {
    stream: R,
    byte: u8,
    bit_offset: usize,
}

impl<R: Buf> AvcBitReader<R> {
    pub fn new(stream: R) -> Self {
        AvcBitReader {
            stream,
            byte: 0,
            bit_offset: 8,
        }
    }

    pub fn read_bit(&mut self) -> Result<u8, Eof> {
        if self.bit_offset == 8 {
            if self.stream.remaining() < 1 {
                return Err(Eof);
            }

            self.byte = self.stream.get_u8();
            self.bit_offset = 0;
        }
        let bit = (self.byte >> (7 - self.bit_offset)) & 0b1;
        self.bit_offset += 1;
        Ok(bit)
    }

    pub fn read_ue(&mut self) -> Result<u64, Eof> {
        self.read_exp_golomb_code()
    }

    pub fn read_ie(&mut self) -> Result<i64, Eof> {
        // https://en.wikipedia.org/wiki/Exponential-Golomb_coding#Extension_to_negative_numbers

        let ue = self.read_ue()?;

        let ie =
            if (ue & 1) == 1 {
                // positive integer
                ((ue + 1) / 2) as i64
            } else {
                // negative integer
                -((ue / 2) as i64)
            };

        Ok(ie)
    }

    fn read_exp_golomb_code(&mut self) -> Result<u64, Eof> {
        let mut leading_zeros = 0;
        while 0 == self.read_bit()? {
            leading_zeros += 1;
        }
        let mut n = 0;
        for _ in 0..leading_zeros {
            let bit = self.read_bit()?;
            n = (n << 1) | u64::from(bit);
        }
        n += 2u64.pow(leading_zeros) - 1;
        Ok(n)
    }
}
