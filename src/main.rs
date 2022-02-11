use anyhow::*;
use image::{ImageBuffer, RgbImage};
use ndarray::{azip, Axis, Zip};
use nshare::RefNdarray3;
use rayon::prelude::*;
use v4l::buffer::Type;
use v4l::frameinterval::FrameIntervalEnum;
use v4l::framesize::{Discrete, Stepwise};
use v4l::io::traits::CaptureStream;
use v4l::video::capture::Parameters;
use v4l::video::Capture;
use v4l::{prelude::*, Format, FourCC, Fraction, FrameInterval};

/// Read images from the first video device.
fn read_images() -> Result<impl Iterator<Item = Result<image::RgbImage>>> {
    let mut dev = Device::new(0).expect("Failed to open device");
    dev.set_format(&Format::new(1920, 1080, FourCC::new(b"MJPG")))?;
    dev.set_params(&Parameters {
        interval: Fraction {
            numerator: 1,
            denominator: 5,
        },
        ..dev.params()?
    })?;
    // println!("intervals: {:?}", dev.enum_frameintervals(FourCC::new(b"MJPG"), 1280, 720)?);

    let mut stream = MmapStream::with_buffers(&mut dev, Type::VideoCapture, 4)
        .expect("Failed to create buffer stream");

    let iter = (0..)
        .into_iter()
        .map(move |_| {
            let (buf, meta) = stream.next().unwrap();
            (buf.to_vec(), meta.clone())
        })
        .step_by(5)
        //.par_bridge()
        .map(|(buf, _meta)| {
            //let im: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> = turbojpeg::decompress_image(&buf)?;
            //let dec = jpeg_decoder::Decoder::new(std::io::Cursor::new(&buf));
            // let image_rgb = dec.decode()?;
            // match dec.info()?.pixel_format {
            //     jpeg_decoder::PixelFormat::RGB24 => (),
            //     pixel_format => { return anyhow!("Unsupported JPEG pixel format {:?}", pixel_format); }
            // };
            let im =
                image::load_from_memory_with_format(&buf, image::ImageFormat::Jpeg)?.into_rgb8();

            // log::debug!(
            //     "Buffer size: {}, inseq: {}, timestamp: {}, image: {:?}",
            //     buf.len(),
            //     meta.sequence,
            //     meta.timestamp,
            //     im.dimensions()
            // );

            Ok(im)
        });
    Ok(iter)
}

struct InterestModel {
    height: usize,
    width: usize,
    buffer: ndarray::Array4<f32>,
    count: usize,
    window: usize,
}
struct Interest {
    original: ndarray::Array3<f32>,
    mean: ndarray::Array3<f32>,
}
impl Interest {
    fn threshold(&self) -> ndarray::Array3<u8> {

        let mut threshold: ndarray::Array3<u8> = ndarray::Array3::zeros(self.original.raw_dim());
        Zip::from(&self.original).and(&self.mean).and(&mut threshold).for_each(|o, m, t| *t += 10 * ((o-m) > 25.0) as u8);
        threshold
    }
    fn overall(&self) -> f32 {
        let mut mean_abs_zscore = 0.0;
        Zip::from(&self.original)
            .and(&self.mean)
            .for_each(|o, m| mean_abs_zscore += ((o - m) > 25.0) as u32 as f32);
        mean_abs_zscore / self.original.len() as f32
    }

    fn dump(&self) -> Result<()> {
        let image_rolled = self.threshold()
            .permuted_axes([1, 2, 0])
            .as_standard_layout()
            .mapv(|x| x as u8);
        let im: ImageBuffer<image::Rgb<u8>, _> = image::ImageBuffer::from_raw(
            self.original.len_of(Axis(2)) as u32,
            self.original.len_of(Axis(1)) as u32,
            image_rolled
                .as_slice()
                .expect("couldn't conform image mean to an image"),
        )
        .expect("Failed to read image for dump");
        let now = chrono::Local::now().timestamp_millis();
        im.save(&format!("original-{}.jpeg", now))?;
        Ok(())
    }
}
impl InterestModel {
    fn new(width: usize, height: usize, window: usize) -> Self {
        Self {
            buffer: ndarray::Array4::zeros((window, 3, height, width)),
            count: 0,
            window,
            width,
            height,
        }
    }
    fn estimate_interest(&mut self, im: &RgbImage) -> Interest {
        let original = im.ref_ndarray3().mapv(|x| x as f32);
        // TODO: Cut down on copies
        if self.count == 0 {
            // Prefill the buffer
            for i in 0..self.window {
                self.buffer.index_axis_mut(Axis(0), i).assign(&original);
            }
        } else {
            // Just do one buffer element
            self.buffer
                .index_axis_mut(Axis(0), self.count % self.window)
                .assign(&original);
        }
        self.count += 1;
        let mean = self.buffer.mean_axis(Axis(0)).expect("Zero size image");

        Interest {
            original,
            mean,
        }
    }
}

fn main() -> Result<()> {
    env_logger::init();
    let mut model = InterestModel::new(1280, 720, 5);
    for im in read_images()? {
        let interest = model.estimate_interest(&im?);
        log::debug!("Interest: {}", interest.overall());
        //interest.dump()?;
    }
    Ok(())
}
