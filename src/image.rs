use crate::protocol::*;
use dither::prelude::*;
use std::path::Path;

pub struct Image {
    image: Img<f64>,
    mean: f64,
}

fn rle_bytes(val: u8, mut counter: u32) -> Vec<u8> {
    let mut compressed = vec![];
    if counter > 0 {
        while counter > 127 {
            let code = (val << 7) | 127;
            compressed.push(code);
            counter -= 127;
        }
        let code = (val << 7) | (counter as u8);
        compressed.push(code);
    }
    compressed
}

impl Image {
    pub fn load(
        path: &Path,
        rotate: bool,
    ) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let image = image::open(path)?;
        let image = if rotate { image.rotate90() } else { image };
        let image = image
            .resize(
                crate::protocol::PIXELS_PER_LINE as u32,
                !0_u32,
                image::imageops::FilterType::CatmullRom,
            )
            .into_rgb8();

        let image: Img<RGB<u8>> = unsafe {
            Img::from_raw_buf(
                image.pixels().map(|p| RGB::from(p.0)).collect(),
                image.width(),
            )
        };

        let image: Img<RGB<f64>> = image.convert_with(|rgb| rgb.convert_with(f64::from));

        let image = image.convert_with(|rgb| rgb.to_chroma_corrected_black_and_white());
        Ok(Self { image, mean: 0.5 })
    }

    pub fn kmean(self) -> Self {
        let mean = self.image.iter().sum::<f64>() / self.image.len() as f64;
        Self {
            image: self.image,
            mean,
        }
    }

    pub fn dither(self, ditherer: &Ditherer) -> Self {
        let image = ditherer.dither(self.image, dither::create_quantize_n_bits_func(1).unwrap());
        Self { image, mean: 0.5 }
    }

    /// Returns a line, bool is true if compressed, false if uncompressed
    pub fn line(
        &self,
        y: u32,
        use_compression: bool,
    ) -> Option<(bool, usize, [u8; crate::protocol::PIXELS_PER_LINE / 8])> {
        if y > self.image.height() {
            return None;
        }

        if use_compression {
            self.line_compressed(y)
        } else {
            self.line_uncompressed(y)
                .map(|(len, data)| (false, len, data))
        }
    }

    pub fn line_compressed(
        &self,
        y: u32,
    ) -> Option<(bool, usize, [u8; crate::protocol::PIXELS_PER_LINE / 8])> {
        let mut compressed = Vec::<u8>::new();

        let mut counter = 0_u32;
        let mut last_val = 2;
        for x in 0..crate::protocol::PIXELS_PER_LINE {
            let x = x as u32;
            let pixel = self.image.get((x, y)).unwrap();
            let val = if pixel > &self.mean { 0 } else { 1 };
            if val == last_val {
                counter = counter + 1
            } else if counter > 0 {
                compressed.extend(rle_bytes(last_val, counter));
                counter = 1;
            }
            last_val = val;
        }
        compressed.extend(rle_bytes(last_val, counter));

        if compressed.len() > crate::protocol::PIXELS_PER_LINE / 8 {
            self.line_uncompressed(y)
                .map(|(len, data)| (false, len, data))
        } else {
            use std::convert::TryInto;
            let len = compressed.len();
            compressed.resize(crate::protocol::PIXELS_PER_LINE / 8, 0);
            Some((true, len, compressed.try_into().unwrap()))
        }
    }

    pub fn line_uncompressed(
        &self,
        y: u32,
    ) -> Option<(usize, [u8; crate::protocol::PIXELS_PER_LINE / 8])> {
        let mut data = [0_u8; crate::protocol::PIXELS_PER_LINE / 8];
        for x in 0..crate::protocol::PIXELS_PER_LINE {
            let x = x as u32;
            let pixel = self.image.get((x, y)).unwrap();
            let val = if pixel > &self.mean { 0 } else { 1 };
            let i = (x / 8) as usize;
            let j = x % 8;
            let current = data[i];
            data[i] = current | (val << j);
        }

        Some((crate::protocol::PIXELS_PER_LINE / 8, data))
    }

    pub fn line_count(&self) -> u32 {
        self.image.height()
    }

    pub fn print(
        &self,
        mode: DrawingMode,
        quality: Quality,
        energy: u16,
        use_compression: bool,
    ) -> Vec<Command> {
        let mut commands = vec![
            Command::SetQuality(quality),
            Command::SetEnergy(energy),
            Command::SetDrawingMode(mode),
        ];

        commands.push(Command::MagicLattice(LatticeType::Start));
        for y in 0..self.line_count() {
            let (compressed, len, pixels) = self.line(y, use_compression).unwrap();
            commands.push(Command::Print(compressed, len, pixels));
        }
        commands.push(Command::MagicLattice(LatticeType::End));

        commands
    }
}
