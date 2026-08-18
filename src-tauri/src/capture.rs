//! Microphone capture: owns the cpal stream on a dedicated thread (cpal
//! streams are !Send) and hands interleaved f32 samples to the pipeline
//! worker through a lock-free ring buffer.
//!
//! The audio callback runs on a real-time thread: it converts and copies
//! into the ring and nothing else — no allocation, no locks, no resampling.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SizedSample};

/// Seconds of interleaved audio the ring buffer can hold. The worker drains
/// every 100ms; this survives multi-second decode stalls without dropping.
const RING_CAPACITY_SECS: usize = 8;

pub struct CaptureSession {
    pub consumer: rtrb::Consumer<f32>,
    pub src_rate: u32,
    pub channels: usize,
    /// Samples the callback had to drop because the ring was full.
    pub dropped: Arc<AtomicUsize>,
}

/// Open the default input device on a new thread. The device config and the
/// ring consumer come back through `ready_tx`; the thread then holds the
/// stream open until `stop` is set.
pub fn spawn(stop: Arc<AtomicBool>, ready_tx: Sender<Result<CaptureSession>>) {
    std::thread::spawn(move || {
        if let Err(e) = run(&stop, &ready_tx) {
            let _ = ready_tx.send(Err(e));
        }
    });
}

fn run(stop: &AtomicBool, ready_tx: &Sender<Result<CaptureSession>>) -> Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .context("no default input device")?;
    let config = device.default_input_config()?;
    let src_rate = config.sample_rate();
    let channels = config.channels() as usize;
    let (producer, consumer) =
        rtrb::RingBuffer::new(src_rate as usize * channels * RING_CAPACITY_SECS);
    let dropped = Arc::new(AtomicUsize::new(0));

    let stream_config: cpal::StreamConfig = config.into();
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            build::<f32>(&device, &stream_config, channels, producer, dropped.clone())
        }
        cpal::SampleFormat::I16 => {
            build::<i16>(&device, &stream_config, channels, producer, dropped.clone())
        }
        cpal::SampleFormat::U16 => {
            build::<u16>(&device, &stream_config, channels, producer, dropped.clone())
        }
        other => anyhow::bail!("unsupported input sample format: {other:?}"),
    }?;
    stream.play()?;
    let _ = ready_tx.send(Ok(CaptureSession {
        consumer,
        src_rate,
        channels,
        dropped,
    }));
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    mut producer: rtrb::Producer<f32>,
    dropped: Arc<AtomicUsize>,
) -> Result<cpal::Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    Ok(device.build_input_stream(
        *config,
        move |data: &[T], _info| {
            // Whole frames only: writing a partial frame when the ring is
            // full would misalign every de-interleaved frame after it.
            let writable = (producer.slots().min(data.len()) / channels) * channels;
            if let Ok(chunk) = producer.write_chunk_uninit(writable) {
                chunk.fill_from_iter(data.iter().map(|s| f32::from_sample(*s)));
            }
            if writable < data.len() {
                dropped.fetch_add(data.len() - writable, Ordering::Relaxed);
            }
        },
        |err| eprintln!("input stream error: {err}"),
        None,
    )?)
}
