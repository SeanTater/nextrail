use v4l::buffer::Type;
use v4l::frameinterval::FrameIntervalEnum;
use v4l::framesize::{Discrete, Stepwise};
use v4l::io::traits::CaptureStream;
use v4l::video::capture::Parameters;
use v4l::{prelude::*, FrameInterval, Format, Fraction, FourCC};
use anyhow::*;
use v4l::video::Capture;
use rayon::prelude::*;

fn main() -> Result<()> {
    let mut dev = Device::new(0).expect("Failed to open device");
    dev.set_format(
        &Format::new(1920, 1080, FourCC::new(b"MJPG"))
    )?;


    let mut stream =
        MmapStream::with_buffers(&mut dev, Type::VideoCapture, 4).expect("Failed to create buffer stream");

    (0..250).into_iter()
    .map(move |_| {
        let (buf, meta) = stream.next().unwrap();
        (buf.to_vec(), meta.clone())
    })
    .par_bridge()
    .map(|(buf, meta)| {
        let im: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> = turbojpeg::decompress_image(&buf)?;

        println!(
            "Buffer size: {}, inseq: {}, timestamp: {}, image: {:?}",
            buf.len(),
            meta.sequence,
            meta.timestamp,
            im.dimensions()
        );
        Ok(())
    }).count();
    Ok(())
}