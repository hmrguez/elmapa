//! Encode each completed GPU readback and stream the MP4 into a temporary file.
use super::{EncoderUpdate, FPS, Frame};
use bevy::{math::UVec2, render::renderer::RenderDevice};
use crossbeam_channel::{Receiver, Sender};
use image::{RgbaImage, imageops::FilterType};
use mp4::{AvcConfig, Mp4Config, Mp4Sample, Mp4Writer, TrackConfig};
use openh264::{
    OpenH264API, Timestamp,
    encoder::{
        BitRate, EncodedBitStream, Encoder, EncoderConfig, FrameRate, FrameType, IntraFramePeriod,
        MatrixCoefficients, UsageType, VuiConfig,
    },
    formats::{RgbaSliceU8, YUVBuffer},
};
use std::{
    error::Error,
    io::{BufWriter, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use tempfile::NamedTempFile;

pub(super) fn run(
    destination: &Path,
    replace: bool,
    source: UVec2,
    input: Receiver<Frame>,
    output: &Sender<EncoderUpdate>,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    encode(destination, replace, source, input, output, cancelled)
        .map_err(|error| error.to_string())
}

fn encode(
    destination: &Path,
    replace: bool,
    source: UVec2,
    input: Receiver<Frame>,
    output: &Sender<EncoderUpdate>,
    cancelled: &AtomicBool,
) -> Result<(), Box<dyn Error>> {
    let size = video_size(source);
    let dimensions = (size.x as usize, size.y as usize);
    let mut yuv = YUVBuffer::new(dimensions.0, dimensions.1);
    // The RGB converter uses limited-range BT.601 coefficients on sRGB pixels.
    let color = VuiConfig::srgb()
        .matrix_coefficients(MatrixCoefficients::Smpte170M)
        .full_range(false);
    let config = EncoderConfig::new()
        .usage_type(UsageType::ScreenContentRealTime)
        .bitrate(BitRate::from_bps(6_000_000))
        .max_frame_rate(FrameRate::from_hz(FPS as f32))
        .intra_frame_period(IntraFramePeriod::from_num_frames(FPS * 2))
        .num_threads(2)
        .skip_frames(false)
        .vui(color);
    let mut encoder = Encoder::with_api_config(OpenH264API::from_source(), config)?;
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temp = NamedTempFile::new_in(parent)?;
    {
        let mut writer = Mp4Writer::write_start(
            BufWriter::new(temp.as_file_mut()),
            &Mp4Config {
                major_brand: (*b"isom").into(),
                minor_version: 512,
                compatible_brands: vec![(*b"isom").into(), (*b"avc1").into(), (*b"mp41").into()],
                timescale: FPS,
            },
        )?;

        for (index, frame) in input.iter().enumerate() {
            if cancelled.load(Ordering::Acquire) {
                return Ok(());
            }
            let pixels = frame_pixels(frame.pixels, source, size)?;
            yuv.read_rgb8(RgbaSliceU8::new(&pixels, dimensions));
            let index = index as u64;
            let packet =
                encoder.encode_at(&yuv, Timestamp::from_millis(index * 1000 / u64::from(FPS)))?;
            let (sample, sps, pps) = avc_sample(&packet, index)?;
            if index == 0 {
                let mut track = TrackConfig::from(AvcConfig {
                    width: size.x as u16,
                    height: size.y as u16,
                    seq_param_set: sps.ok_or("the encoder did not supply an SPS")?,
                    pic_param_set: pps.ok_or("the encoder did not supply a PPS")?,
                });
                track.timescale = FPS;
                writer.add_track(&track)?;
            }
            writer.write_sample(1, &sample)?;
            if frame.last {
                writer.write_end()?;
                writer.into_writer().flush()?;
                temp.as_file().sync_all()?;
                if !cancelled.load(Ordering::Acquire) {
                    let saved = if replace {
                        temp.persist(destination)
                    } else {
                        temp.persist_noclobber(destination)
                    };
                    saved.map_err(|error| error.error)?;
                }
                return Ok(());
            }
            if output.send(EncoderUpdate::Encoded).is_err() {
                return Ok(());
            }
        }
    }
    Ok(())
}

fn video_size(source: UVec2) -> UVec2 {
    let scale = (1920. / source.x as f64)
        .min(1080. / source.y as f64)
        .min(1.);
    let even = |size: u32| ((size as f64 * scale).round() as u32 & !1).max(2);
    UVec2::new(even(source.x), even(source.y))
}

fn frame_pixels(mut pixels: Vec<u8>, source: UVec2, size: UVec2) -> Result<Vec<u8>, &'static str> {
    let row = source.x as usize * 4;
    let stride = RenderDevice::align_copy_bytes_per_row(row);
    let height = source.y as usize;
    if pixels.len() != stride * height {
        return Err("the map renderer returned an unexpected frame size");
    }
    if stride != row {
        for y in 1..height {
            let offset = y * stride;
            pixels.copy_within(offset..offset + row, y * row);
        }
        pixels.truncate(row * height);
    }
    if source == size {
        return Ok(pixels);
    }
    let image = RgbaImage::from_raw(source.x, source.y, pixels).ok_or("invalid map image")?;
    Ok(image::imageops::resize(&image, size.x, size.y, FilterType::Triangle).into_raw())
}

type AvcSample = (Mp4Sample, Option<Vec<u8>>, Option<Vec<u8>>);

fn avc_sample(packet: &EncodedBitStream, index: u64) -> Result<AvcSample, &'static str> {
    let (mut sps, mut pps) = (None, None);
    let mut bytes = Vec::new();
    for layer in (0..packet.num_layers()).filter_map(|i| packet.layer(i)) {
        for unit in (0..layer.nal_count()).filter_map(|i| layer.nal_unit(i)) {
            let nal = unit
                .strip_prefix(&[0, 0, 0, 1])
                .or_else(|| unit.strip_prefix(&[0, 0, 1]))
                .filter(|nal| !nal.is_empty())
                .ok_or("invalid H.264 NAL unit")?;
            // avc1 stores parameter sets in the track and length-prefixes other units.
            match nal[0] & 0x1f {
                7 if index == 0 => sps = Some(nal.to_vec()),
                8 if index == 0 => pps = Some(nal.to_vec()),
                7 | 8 => {}
                _ => {
                    bytes.extend_from_slice(&(nal.len() as u32).to_be_bytes());
                    bytes.extend_from_slice(nal);
                }
            }
        }
    }
    if bytes.is_empty() {
        return Err("the encoder skipped a replay frame");
    }
    let sample = Mp4Sample {
        start_time: index,
        duration: 1,
        rendering_offset: 0,
        is_sync: packet.frame_type() == FrameType::IDR,
        bytes: bytes.into(),
    };
    Ok((sample, sps, pps))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs::File, io::BufReader, time::Duration};

    #[test]
    fn writes_one_h264_track_with_fixed_frame_timing() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("replay.mp4");
        let source = UVec2::new(131, 95);
        let stride = RenderDevice::align_copy_bytes_per_row(source.x as usize * 4);
        let (frames, input) = crossbeam_channel::bounded(3);
        let (output, _updates) = crossbeam_channel::unbounded();
        for frame in 0..3 {
            frames
                .send(Frame {
                    pixels: vec![96 + frame * 32; stride * source.y as usize],
                    last: frame == 2,
                })
                .unwrap();
        }
        encode(
            &path,
            false,
            source,
            input,
            &output,
            &AtomicBool::new(false),
        )
        .unwrap();

        let file = File::open(path).unwrap();
        let bytes = file.metadata().unwrap().len();
        let mut reader = mp4::Mp4Reader::read_header(BufReader::new(file), bytes).unwrap();
        assert_eq!(reader.tracks().len(), 1);
        assert_eq!(reader.duration(), Duration::from_millis(100));
        let track = &reader.tracks()[&1];
        assert_eq!(track.media_type().unwrap(), mp4::MediaType::H264);
        assert_eq!((track.width(), track.height()), (130, 94));
        assert_eq!(track.sample_count(), 3);
        assert!(reader.read_sample(1, 1).unwrap().unwrap().is_sync);
        let last = reader.read_sample(1, 3).unwrap().unwrap();
        assert_eq!((last.start_time, last.duration), (2, 1));
        assert_eq!(directory.path().read_dir().unwrap().count(), 1);
    }

    #[test]
    fn abandoned_replay_preserves_destination_and_cleans_staging_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("replay.mp4");
        std::fs::write(&path, b"existing video").unwrap();
        let (frames, input) = crossbeam_channel::bounded(1);
        let (output, _updates) = crossbeam_channel::unbounded();
        drop(frames);
        encode(
            &path,
            true,
            UVec2::splat(64),
            input,
            &output,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"existing video");
        assert_eq!(directory.path().read_dir().unwrap().count(), 1);
    }
}
