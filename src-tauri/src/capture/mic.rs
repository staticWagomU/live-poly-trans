//! Microphone capture: owns the cpal stream on a dedicated thread (cpal
//! streams are !Send).
//!
//! The audio callback runs on a real-time thread: it converts and copies
//! into the ring and nothing else — no allocation, no locks, no resampling.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SizedSample};

use super::{Anchor, CaptureSession, ANCHOR_CAPACITY, RING_CAPACITY_SECS};

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
    let (times_tx, times) = rtrb::RingBuffer::new(ANCHOR_CAPACITY);
    let dropped = Arc::new(AtomicUsize::new(0));
    let error = Arc::new(Mutex::new(None));

    let stream_config: cpal::StreamConfig = config.into();
    let sink = Sink {
        producer,
        times: times_tx,
        channels,
        frames: 0,
        dropped: dropped.clone(),
    };
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build::<f32>(&device, &stream_config, sink, error.clone()),
        cpal::SampleFormat::I16 => build::<i16>(&device, &stream_config, sink, error.clone()),
        cpal::SampleFormat::U16 => build::<u16>(&device, &stream_config, sink, error.clone()),
        other => anyhow::bail!("unsupported input sample format: {other:?}"),
    }?;
    stream.play()?;
    let _ = ready_tx.send(Ok(CaptureSession {
        consumer,
        times,
        src_rate,
        channels,
        dropped,
        error,
        output_device_switch: None,
    }));
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

/// Everything the audio callback owns. Kept together so the callback body
/// stays a straight copy: no allocation, no locks.
struct Sink {
    producer: rtrb::Producer<f32>,
    times: rtrb::Producer<Anchor>,
    channels: usize,
    /// Frames written to the ring so far — the anchors' frame of reference.
    frames: u64,
    dropped: Arc<AtomicUsize>,
}

impl Sink {
    fn consume(&mut self, samples: impl ExactSizeIterator<Item = f32>, captured_nanos: u64) {
        let len = samples.len();
        // Whole frames only: writing a partial frame when the ring is
        // full would misalign every de-interleaved frame after it.
        let writable = (self.producer.slots().min(len) / self.channels) * self.channels;
        if writable > 0 {
            // Stamped before the write so the reader always finds an anchor
            // at or before the audio it is looking at.
            let _ = self.times.push(Anchor {
                frame: self.frames,
                nanos: captured_nanos,
            });
            if let Ok(chunk) = self.producer.write_chunk_uninit(writable) {
                chunk.fill_from_iter(samples.take(writable));
            }
            self.frames += (writable / self.channels) as u64;
        }
        if writable < len {
            self.dropped.fetch_add(len - writable, Ordering::Relaxed);
        }
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut sink: Sink,
    error: Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    Ok(device.build_input_stream(
        *config,
        move |data: &[T], info: &cpal::InputCallbackInfo| {
            // `capture` is the callback's host time less the input latency:
            // when these frames were actually picked up, not when we woke up.
            let captured = info.timestamp().capture.as_nanos() as u64;
            sink.consume(data.iter().map(|s| f32::from_sample(*s)), captured);
        },
        move |err| {
            // First error wins; try_lock so this callback never blocks.
            if let Ok(mut slot) = error.try_lock() {
                slot.get_or_insert_with(|| err.to_string());
            }
            eprintln!("input stream error: {err}");
        },
        None,
    )?)
}
