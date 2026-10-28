/*
 * Copyright 2009 ZXing authors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//package com.google.zxing;

use std::borrow::Cow;

use crate::common::Result;
use crate::{Exceptions, LuminanceSource};

/**
 * This class is used to help decode images from files which arrive as RGB data from
 * an ARGB pixel array. It does not support rotation.
 *
 * @author dswitkin@google.com (Daniel Switkin)
 * @author Betaminos
 */
#[derive(Debug, Clone)]
pub struct RGBLuminanceSource {
    luminances: Box<[u8]>,
    dataWidth: usize,
    dataHeight: usize,
    width: usize,
    height: usize,
    invert: bool,
}

impl LuminanceSource for RGBLuminanceSource {
    const SUPPORTS_CROP: bool = true;

    /// gets a row, returns an empty row if we are out of bounds.
    fn get_row(&self, y: usize) -> Option<Cow<'_, [u8]>> {
        if y >= self.get_height() {
            return None;
        }
        let width = self.get_width();

        let offset = (y) * self.dataWidth;

        if self.invert {
            Some(Cow::Owned(self.invert_block_of_bytes(
                self.luminances[offset..offset + width].to_vec(),
            )))
        } else {
            Some(Cow::Borrowed(&self.luminances[offset..offset + width]))
        }
    }

    fn get_column(&self, _x: usize) -> Cow<'_, [u8]> {
        unimplemented!()
    }

    fn get_matrix(&self) -> Cow<'_, [u8]> {
        if self.invert {
            Cow::Owned(self.invert_block_of_bytes(self.luminances.to_vec()))
        } else {
            Cow::Borrowed(&self.luminances)
        }
    }

    fn get_width(&self) -> usize {
        self.width
    }

    fn get_height(&self) -> usize {
        self.height
    }

    fn crop(&self, left: usize, top: usize, width: usize, height: usize) -> Result<Self> {
        RGBLuminanceSource::new_complex(
            self.luminances
                .chunks_exact(self.dataWidth)
                .skip(top)
                .take(height)
                .flat_map(|row| row.iter().skip(left).take(width))
                .copied()
                .collect(),
            // the cropped pixels were copied into a compact buffer, so its data
            // dimensions are the crop dimensions, not those of the source image
            width,
            height,
            width,
            height,
        )
        .map_err(|_| Exceptions::UNSUPPORTED_OPERATION)
    }

    fn invert(&mut self) {
        self.invert = !self.invert;
    }

    fn get_luma8_point(&self, x: usize, y: usize) -> u8 {
        let _width = self.get_width();
        let row_offset = (y) * self.dataWidth;
        let col_offset = x;

        self.luminances[row_offset + col_offset]
    }
}

impl RGBLuminanceSource {
    pub fn new_with_width_height_pixels(
        width: usize,
        height: usize,
        pixels: &[u32],
    ) -> Result<Self> {
        let dataWidth = width;
        let dataHeight = height;

        // In order to measure pure decoding speed, we convert the entire image to a greyscale array
        // up front, which is the same as the Y channel of the YUVLuminanceSource in the real app.
        //
        // Total number of pixels suffices, can ignore shape
        let size = width * height;

        if size != pixels.len() {
            return Err(Exceptions::illegal_argument_with(
                "Dimensions do not match the data length.",
            ));
        }

        let mut luminances: Vec<u8> = vec![0; size];
        for offset in 0..size {
            let pixel = pixels[offset];
            let r = (pixel >> 16) & 0xff; // red
            let g2 = (pixel >> 7) & 0x1fe; // 2 * green
            let b = pixel & 0xff; // blue
            // Calculate green-favouring average cheaply
            luminances[offset] = ((r + g2 + b) / 4) as u8;
        }
        Ok(Self {
            luminances: luminances.into_boxed_slice(),
            dataWidth,
            dataHeight,
            width,
            height,
            invert: false,
        })
    }

    fn new_complex(
        pixels: Box<[u8]>,
        data_width: usize,
        data_height: usize,
        width: usize,
        height: usize,
    ) -> Result<Self> {
        if width > data_width || height > data_height {
            return Err(Exceptions::illegal_argument_with(
                "Crop rectangle does not fit within image data.",
            ));
        }
        Ok(Self {
            luminances: pixels,
            dataWidth: data_width,
            dataHeight: data_height,
            width,
            height,
            invert: false,
        })
    }
}

#[cfg(test)]
mod crop_tests {
    use super::*;

    /// 4x3 image whose luminance equals `10 * y + x` (gray pixels, r = g = b).
    fn gradient() -> RGBLuminanceSource {
        let pixels: Vec<u32> = (0..3u32)
            .flat_map(|y| {
                (0..4u32).map(move |x| {
                    let v = 10 * y + x;
                    (v << 16) | (v << 8) | v
                })
            })
            .collect();
        RGBLuminanceSource::new_with_width_height_pixels(4, 3, &pixels).unwrap()
    }

    #[test]
    fn cropped_rows_come_from_the_cropped_region() {
        let cropped = gradient().crop(1, 1, 2, 2).unwrap();
        assert_eq!(cropped.get_width(), 2);
        assert_eq!(cropped.get_height(), 2);
        assert_eq!(&*cropped.get_row(0).unwrap(), &[11, 12]);
        assert_eq!(&*cropped.get_row(1).unwrap(), &[21, 22]);
        assert!(cropped.get_row(2).is_none());
    }

    #[test]
    fn cropped_points_and_matrix_agree_with_rows() {
        let cropped = gradient().crop(1, 1, 2, 2).unwrap();
        assert_eq!(cropped.get_luma8_point(0, 0), 11);
        assert_eq!(cropped.get_luma8_point(1, 1), 22);
        assert_eq!(&*cropped.get_matrix(), &[11, 12, 21, 22]);
    }

    #[test]
    fn crop_of_a_crop_is_consistent() {
        let cropped = gradient().crop(1, 0, 3, 3).unwrap().crop(1, 1, 2, 2).unwrap();
        assert_eq!(&*cropped.get_row(0).unwrap(), &[12, 13]);
        assert_eq!(&*cropped.get_row(1).unwrap(), &[22, 23]);
    }
}
