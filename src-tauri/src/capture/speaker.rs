//! System-audio capture via a CoreAudio Process Tap (macOS 14.2+).
//!
//! Verified by the Step 0 spike (docs/step0-tap-results.md); that document
//! also records why every failure here is silent rather than an error:
//!
//! - Without the System Audio Recording permission every call still returns
//!   noErr and the IO block still fires — the buffers are just zero-filled.
//!   The app must therefore be a signed bundle with
//!   NSAudioCaptureUsageDescription, and `cargo run` will capture silence.
//! - An idle output device runs no IO cycle at all, so a working tap
//!   delivers no callbacks until something plays.

use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::AllocAnyThread;
use objc2_core_audio::{
    kAudioAggregateDeviceIsPrivateKey, kAudioAggregateDeviceIsStackedKey,
    kAudioAggregateDeviceMainSubDeviceKey, kAudioAggregateDeviceNameKey,
    kAudioAggregateDeviceSubDeviceListKey, kAudioAggregateDeviceTapAutoStartKey,
    kAudioAggregateDeviceTapListKey, kAudioAggregateDeviceUIDKey, kAudioDevicePropertyDeviceUID,
    kAudioHardwarePropertyDefaultOutputDevice, kAudioObjectPropertyElementMain,
    kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject, kAudioSubDeviceUIDKey,
    kAudioSubTapDriftCompensationKey, kAudioSubTapUIDKey, kAudioTapPropertyFormat,
    AudioDeviceCreateIOProcIDWithBlock, AudioDeviceDestroyIOProcID, AudioDeviceIOProcID,
    AudioDeviceStart, AudioDeviceStop, AudioHardwareCreateAggregateDevice,
    AudioHardwareCreateProcessTap, AudioHardwareDestroyAggregateDevice,
    AudioHardwareDestroyProcessTap, AudioObjectGetPropertyData, AudioObjectID,
    AudioObjectPropertyAddress, CATapDescription,
};
use objc2_core_audio_types::{
    kAudioFormatFlagIsFloat, kAudioFormatFlagIsNonInterleaved, kAudioFormatLinearPCM,
    AudioBufferList, AudioStreamBasicDescription, AudioTimeStamp,
};
use objc2_core_foundation::CFDictionary;
use objc2_foundation::{NSArray, NSMutableDictionary, NSNumber, NSString};

use super::{Anchor, CaptureSession, ANCHOR_CAPACITY, RING_CAPACITY_SECS};

/// Written only by the CoreAudio IO block, which the HAL runs serially on
/// the dispatch queue we hand it. Nothing else touches this state, so the
/// unsynchronised access is sound and the callback stays allocation- and
/// lock-free like the cpal one.
struct IoState {
    producer: rtrb::Producer<f32>,
    times: rtrb::Producer<Anchor>,
    /// Frames written to the ring so far — the anchors' frame of reference.
    frames: u64,
}

struct IoSink {
    state: std::cell::UnsafeCell<IoState>,
    channels: usize,
    dropped: Arc<AtomicUsize>,
}

// SAFETY: see IoState's doc comment — access is serialised by the HAL.
unsafe impl Send for IoSink {}
unsafe impl Sync for IoSink {}

impl IoSink {
    /// Copy one IO cycle's input into the ring. Mirrors mic.rs: whole frames
    /// only, since a partial frame would misalign every later de-interleave.
    fn consume(&self, in_data: NonNull<AudioBufferList>, captured_nanos: u64) {
        let abl = unsafe { in_data.as_ref() };
        let buffers = unsafe {
            std::slice::from_raw_parts(abl.mBuffers.as_ptr(), abl.mNumberBuffers as usize)
        };
        let state = unsafe { &mut *self.state.get() };
        for buf in buffers {
            if buf.mData.is_null() {
                continue;
            }
            let samples = unsafe {
                std::slice::from_raw_parts(
                    buf.mData as *const f32,
                    buf.mDataByteSize as usize / std::mem::size_of::<f32>(),
                )
            };
            let writable =
                (state.producer.slots().min(samples.len()) / self.channels) * self.channels;
            if writable > 0 {
                // Stamped before the write so the reader always finds an
                // anchor at or before the audio it is looking at.
                let _ = state.times.push(Anchor {
                    frame: state.frames,
                    nanos: captured_nanos,
                });
                if let Ok(chunk) = state.producer.write_chunk_uninit(writable) {
                    chunk.fill_from_iter(samples[..writable].iter().copied());
                }
                state.frames += (writable / self.channels) as u64;
            }
            if writable < samples.len() {
                self.dropped
                    .fetch_add(samples.len() - writable, Ordering::Relaxed);
            }
        }
    }
}

/// mach ticks → nanoseconds. The ratio is fixed for the machine, so it is
/// read once and reused; a tap callback must not make system calls.
fn host_time_nanos(ticks: u64) -> u64 {
    static SCALE: std::sync::OnceLock<(u64, u64)> = std::sync::OnceLock::new();
    let (numer, denom) = *SCALE.get_or_init(|| {
        let mut info = mach2::mach_time::mach_timebase_info { numer: 0, denom: 0 };
        // A zero denominator would mean a broken kernel; 1/1 at least keeps
        // the arithmetic sane.
        if unsafe { mach2::mach_time::mach_timebase_info(&mut info) } != 0 || info.denom == 0 {
            return (1, 1);
        }
        (info.numer as u64, info.denom as u64)
    });
    (ticks as u128 * numer as u128 / denom as u128) as u64
}

/// Start tapping system audio on a new thread. The tap format and the ring
/// consumer come back through `ready_tx`; the thread then keeps the tap
/// alive until `stop` is set.
pub fn spawn(stop: Arc<AtomicBool>, ready_tx: Sender<Result<CaptureSession>>) {
    std::thread::spawn(move || {
        if let Err(e) = run(&stop, &ready_tx) {
            let _ = ready_tx.send(Err(e));
        }
    });
}

fn run(stop: &AtomicBool, ready_tx: &Sender<Result<CaptureSession>>) -> Result<()> {
    let desc = unsafe {
        CATapDescription::initStereoGlobalTapButExcludeProcesses(
            CATapDescription::alloc(),
            &NSArray::new(),
        )
    };
    unsafe {
        desc.setName(&NSString::from_str("Kikimimic"));
        desc.setPrivate(true);
    }
    let mut tap_id: AudioObjectID = 0;
    check(
        unsafe { AudioHardwareCreateProcessTap(Some(&desc), &mut tap_id) },
        "create process tap (is the app bundle signed with \
         NSAudioCaptureUsageDescription?)",
    )?;
    let _tap = TapGuard(tap_id);

    let asbd = tap_format(tap_id)?;
    // The IO block reads the buffers as interleaved f32 frames; any other
    // format would come out as garbage rather than an error, so refuse it
    // loudly here.
    anyhow::ensure!(
        asbd.mFormatID == kAudioFormatLinearPCM
            && asbd.mFormatFlags & kAudioFormatFlagIsFloat != 0
            && asbd.mFormatFlags & kAudioFormatFlagIsNonInterleaved == 0,
        "tap format is not interleaved f32 PCM (format {:#x}, flags {:#x})",
        asbd.mFormatID,
        asbd.mFormatFlags,
    );
    let channels = (asbd.mChannelsPerFrame as usize).max(1);
    let src_rate = asbd.mSampleRate as u32;
    anyhow::ensure!(src_rate > 0, "tap reported a zero sample rate");

    let agg_id = create_aggregate(&desc)?;
    let _agg = AggregateGuard(agg_id);

    let (producer, consumer) =
        rtrb::RingBuffer::new(src_rate as usize * channels * RING_CAPACITY_SECS);
    let (times_tx, times) = rtrb::RingBuffer::new(ANCHOR_CAPACITY);
    let dropped = Arc::new(AtomicUsize::new(0));
    let sink = Arc::new(IoSink {
        state: std::cell::UnsafeCell::new(IoState {
            producer,
            times: times_tx,
            frames: 0,
        }),
        channels,
        dropped: dropped.clone(),
    });

    let block = block2::RcBlock::new(
        move |_now: NonNull<AudioTimeStamp>,
              in_data: NonNull<AudioBufferList>,
              in_time: NonNull<AudioTimeStamp>,
              _out_data: NonNull<AudioBufferList>,
              _out_time: NonNull<AudioTimeStamp>| {
            // The HAL's host time for this input buffer: the same mach clock
            // cpal stamps the mic with, which is what puts both lanes on one
            // timeline.
            let captured = host_time_nanos(unsafe { in_time.as_ref() }.mHostTime);
            sink.consume(in_data, captured);
        },
    );
    // A nil queue reportedly fails to register the block on macOS 26, so
    // always hand the HAL an explicit one.
    let queue = dispatch2::DispatchQueue::new("dev.wagomu.kkm.speaker-io", None);
    let mut proc_id: AudioDeviceIOProcID = None;
    check(
        unsafe {
            AudioDeviceCreateIOProcIDWithBlock(
                NonNull::from(&mut proc_id),
                agg_id,
                Some(&queue),
                block2::RcBlock::as_ptr(&block) as _,
            )
        },
        "create IO block",
    )?;
    // Aggregate composition is asynchronous; starting IO into a half-built
    // device is what "succeeds" and then delivers nothing.
    std::thread::sleep(Duration::from_millis(300));
    check(unsafe { AudioDeviceStart(agg_id, proc_id) }, "start tap IO")?;
    let _io = IoProcGuard { agg_id, proc_id };

    let _ = ready_tx.send(Ok(CaptureSession {
        consumer,
        times,
        src_rate,
        channels,
        dropped,
        // The HAL has no error callback for taps; device changes are a
        // Step 3 follow-up (docs/step0-tap-results.md).
        error: Arc::new(Mutex::new(None)),
    }));
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn check(status: i32, what: &str) -> Result<()> {
    anyhow::ensure!(status == 0, "{what} failed: OSStatus {status}");
    Ok(())
}

/// The tap's stream format: what the IO block will actually deliver.
fn tap_format(tap_id: AudioObjectID) -> Result<AudioStreamBasicDescription> {
    let mut asbd: AudioStreamBasicDescription = unsafe { std::mem::zeroed() };
    read_property(
        tap_id,
        kAudioTapPropertyFormat,
        &mut asbd,
        "read tap stream format",
    )?;
    Ok(asbd)
}

/// A private aggregate device carrying the tap, clocked by the default
/// output device (which appears as both main sub-device and sub-device).
fn create_aggregate(desc: &CATapDescription) -> Result<AudioObjectID> {
    let out_uid = default_output_uid()?;
    let sub_dev = NSMutableDictionary::<NSString, AnyObject>::new();
    let sub_devs = unsafe {
        set(&sub_dev, kAudioSubDeviceUIDKey, &out_uid);
        NSArray::from_retained_slice(&[sub_dev])
    };
    let sub_tap = NSMutableDictionary::<NSString, AnyObject>::new();
    let taps = unsafe {
        set(&sub_tap, kAudioSubTapUIDKey, &desc.UUID().UUIDString());
        set(
            &sub_tap,
            kAudioSubTapDriftCompensationKey,
            &NSNumber::new_bool(true),
        );
        NSArray::from_retained_slice(&[sub_tap])
    };

    let agg = NSMutableDictionary::<NSString, AnyObject>::new();
    unsafe {
        set(
            &agg,
            kAudioAggregateDeviceNameKey,
            &NSString::from_str("Kikimimic speaker lane"),
        );
        set(
            &agg,
            kAudioAggregateDeviceUIDKey,
            &NSString::from_str("dev.wagomu.kikimimic.speaker-lane"),
        );
        set(
            &agg,
            kAudioAggregateDeviceIsPrivateKey,
            &NSNumber::new_bool(true),
        );
        set(
            &agg,
            kAudioAggregateDeviceIsStackedKey,
            &NSNumber::new_bool(false),
        );
        set(
            &agg,
            kAudioAggregateDeviceTapAutoStartKey,
            &NSNumber::new_bool(true),
        );
        set(&agg, kAudioAggregateDeviceMainSubDeviceKey, &out_uid);
        set(&agg, kAudioAggregateDeviceSubDeviceListKey, &sub_devs);
        set(&agg, kAudioAggregateDeviceTapListKey, &taps);
    }
    // NSDictionary is toll-free bridged to CFDictionary.
    let cf: &CFDictionary = unsafe { &*(Retained::as_ptr(&agg) as *const CFDictionary) };
    let mut agg_id: AudioObjectID = 0;
    check(
        unsafe { AudioHardwareCreateAggregateDevice(cf, NonNull::from(&mut agg_id)) },
        "create aggregate device",
    )?;
    Ok(agg_id)
}

fn default_output_uid() -> Result<Retained<NSString>> {
    let mut dev: AudioObjectID = 0;
    read_property(
        kAudioObjectSystemObject as AudioObjectID,
        kAudioHardwarePropertyDefaultOutputDevice,
        &mut dev,
        "get default output device",
    )?;
    // CFStringRef, toll-free bridged to NSString; the property returns +1
    // and Retained::from_raw takes over that reference.
    let mut uid: *mut NSString = std::ptr::null_mut();
    read_property(
        dev,
        kAudioDevicePropertyDeviceUID,
        &mut uid,
        "get default output device UID",
    )?;
    unsafe { Retained::from_raw(uid) }.context("default output device UID was null")
}

fn read_property<T>(object: AudioObjectID, selector: u32, out: &mut T, what: &str) -> Result<()> {
    let mut addr = AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };
    let mut size = std::mem::size_of::<T>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            NonNull::from(&mut addr),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::new(out as *mut T as *mut c_void).expect("out is a live reference"),
        )
    };
    check(status, what)
}

/// NSDictionary insert with a CoreAudio `&CStr` key.
unsafe fn set(
    dict: &NSMutableDictionary<NSString, AnyObject>,
    key: &std::ffi::CStr,
    value: &AnyObject,
) {
    let key = NSString::from_str(key.to_str().expect("CoreAudio keys are ASCII"));
    dict.setObject_forKey(value, ProtocolObject::from_ref(&*key));
}

/// The HAL objects outlive Rust scopes unless explicitly destroyed, and a
/// leaked tap keeps showing up in the audio system. Guards make the teardown
/// order (IO → aggregate → tap) survive early returns.
struct IoProcGuard {
    agg_id: AudioObjectID,
    proc_id: AudioDeviceIOProcID,
}

impl Drop for IoProcGuard {
    fn drop(&mut self) {
        unsafe {
            AudioDeviceStop(self.agg_id, self.proc_id);
            AudioDeviceDestroyIOProcID(self.agg_id, self.proc_id);
        }
    }
}

struct AggregateGuard(AudioObjectID);

impl Drop for AggregateGuard {
    fn drop(&mut self) {
        unsafe { AudioHardwareDestroyAggregateDevice(self.0) };
    }
}

struct TapGuard(AudioObjectID);

impl Drop for TapGuard {
    fn drop(&mut self) {
        unsafe { AudioHardwareDestroyProcessTap(self.0) };
    }
}
