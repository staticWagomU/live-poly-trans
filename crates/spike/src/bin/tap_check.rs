//! Step 0 spike: can Rust capture system audio via a CoreAudio Process Tap
//! (macOS 14.2+)? This is the last open question of ADR-153803 — if it works,
//! the speaker lane (plan.md Step 3) needs no Swift helper at all.
//!
//! Success criteria: tap + aggregate device created, the IOProc fires, and
//! the captured PCM is non-silent while audio is playing.
//!
//! RESULT (macOS 26.5, M4 Pro): PASS — 48kHz stereo system audio captured
//! from Rust, no Swift helper needed. Two conditions are non-negotiable and
//! both fail *silently* when unmet, which is why `scripts/tap-check.sh`
//! exists rather than a plain `cargo run`:
//!
//! 1. Run from a signed .app bundle launched with `open`. A bare CLI binary
//!    makes the terminal the TCC "responsible process", and the System Audio
//!    Recording permission is then withheld as zero-filled buffers — every
//!    call still returns noErr and the IOProc still fires at full cadence.
//!    An identical Swift implementation behaved the same, ruling out FFI.
//! 2. Something must be playing. An idle output device runs no IO cycle, so
//!    the tap gets no callbacks at all and looks broken when it is not.
//!
//! Usage:
//!   scripts/tap-check.sh [seconds]    # bundles, signs, plays audio, runs
//! `tap-check out` bisects: same IO path on the plain output device, no tap.
//! Writes /tmp/kkm-tap-check.wav for listening back.

use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::AllocAnyThread;
use objc2_core_audio::{
    kAudioAggregateDeviceIsPrivateKey, kAudioAggregateDeviceMainSubDeviceKey,
    kAudioAggregateDeviceNameKey, kAudioAggregateDeviceSubDeviceListKey,
    kAudioAggregateDeviceTapAutoStartKey, kAudioAggregateDeviceTapListKey,
    kAudioAggregateDeviceUIDKey, kAudioDevicePropertyDeviceUID,
    kAudioHardwarePropertyDefaultOutputDevice, kAudioObjectPropertyElementMain,
    kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject, kAudioSubDeviceUIDKey,
    kAudioSubTapDriftCompensationKey, kAudioSubTapUIDKey, kAudioTapPropertyFormat,
    AudioDeviceCreateIOProcIDWithBlock, AudioDeviceDestroyIOProcID, AudioDeviceStart,
    AudioDeviceStop, AudioHardwareCreateAggregateDevice, AudioHardwareCreateProcessTap,
    AudioHardwareDestroyAggregateDevice, AudioHardwareDestroyProcessTap,
    AudioObjectGetPropertyData, AudioObjectID, AudioObjectPropertyAddress, CATapDescription,
};
use objc2_core_audio_types::{AudioBufferList, AudioStreamBasicDescription, AudioTimeStamp};
use objc2_core_foundation::CFDictionary;
use objc2_foundation::{NSArray, NSMutableDictionary, NSNumber, NSString};

const WAV_PATH: &str = "/tmp/kkm-tap-check.wav";
/// `open`-launched bundles have no terminal to print to, so the verdict is
/// also written here for the runner script to show.
const REPORT_PATH: &str = "/tmp/kkm-tap-check.txt";

/// Shared with the IO block. A Mutex in an audio callback is fine for a
/// spike; the production lane will reuse the rtrb ring from capture.rs.
struct Sink {
    samples: Mutex<Vec<f32>>,
    callbacks: AtomicUsize,
}

impl Sink {
    fn consume(&self, in_data: NonNull<AudioBufferList>) {
        self.callbacks.fetch_add(1, Ordering::Relaxed);
        let abl = unsafe { in_data.as_ref() };
        let buffers = unsafe {
            std::slice::from_raw_parts(abl.mBuffers.as_ptr(), abl.mNumberBuffers as usize)
        };
        let mut all = self.samples.lock().unwrap();
        for buf in buffers {
            if buf.mData.is_null() {
                continue;
            }
            let n = buf.mDataByteSize as usize / std::mem::size_of::<f32>();
            all.extend_from_slice(unsafe {
                std::slice::from_raw_parts(buf.mData as *const f32, n)
            });
        }
    }
}

fn check(status: i32, what: &str) -> Result<()> {
    anyhow::ensure!(status == 0, "{what} failed: OSStatus {status}");
    Ok(())
}

fn nsstr(key: &std::ffi::CStr) -> Retained<NSString> {
    NSString::from_str(key.to_str().expect("CoreAudio keys are ASCII"))
}

fn main() -> Result<()> {
    // Bisect mode: `tap-check out [secs]` attaches the IOProc straight to
    // the default output device (no tap, no aggregate). If that fires, HAL
    // IO works here and the problem is the tap/aggregate composition.
    if std::env::args().nth(1).as_deref() == Some("out") {
        let secs = std::env::args()
            .nth(2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(3);
        let (dev, _uid) = default_output_device()?;
        println!("default output device: AudioObjectID {dev}");
        let asbd: AudioStreamBasicDescription = unsafe { std::mem::zeroed() };
        return capture(dev, &asbd, secs);
    }
    let secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);

    // 1. Tap description: stereo mix of every process's output. Unmuted, so
    // the user keeps hearing their audio while we tap it.
    let desc = unsafe {
        CATapDescription::initStereoGlobalTapButExcludeProcesses(
            CATapDescription::alloc(),
            &NSArray::new(),
        )
    };
    unsafe {
        desc.setName(&NSString::from_str("kkm-tap-check"));
        desc.setPrivate(true);
    }

    // The TCC "System Audio Recording" prompt fires here on first run.
    let mut tap_id: AudioObjectID = 0;
    let status = unsafe { AudioHardwareCreateProcessTap(Some(&desc), &mut tap_id) };
    check(
        status,
        "AudioHardwareCreateProcessTap (audio-capture permission?)",
    )?;
    println!("tap created: AudioObjectID {tap_id}");

    let result = run_with_tap(&desc, tap_id, secs);
    unsafe { AudioHardwareDestroyProcessTap(tap_id) };
    result
}

fn run_with_tap(desc: &CATapDescription, tap_id: AudioObjectID, secs: u64) -> Result<()> {
    // 2. The tap's stream format tells us the sample rate / channel count
    // the IOProc will deliver.
    let mut addr = AudioObjectPropertyAddress {
        mSelector: kAudioTapPropertyFormat,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };
    let mut asbd: AudioStreamBasicDescription = unsafe { std::mem::zeroed() };
    let mut size = std::mem::size_of::<AudioStreamBasicDescription>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            tap_id,
            NonNull::from(&mut addr),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::new(&mut asbd as *mut _ as *mut c_void).unwrap(),
        )
    };
    check(status, "read kAudioTapPropertyFormat")?;
    println!(
        "tap format: {} Hz, {} ch, formatID {:#010x}, flags {:#010x}",
        asbd.mSampleRate, asbd.mChannelsPerFrame, asbd.mFormatID, asbd.mFormatFlags
    );

    // 3. A private aggregate device carrying the tap, with the default
    // output device as its main sub-device to act as clock source. A
    // tap-only aggregate also delivers callbacks here, but every writeup of
    // this API recommends the real sub-device, so keep it.
    let (_, out_uid) = default_output_device()?;
    let sub_dev = NSMutableDictionary::<NSString, AnyObject>::new();
    let sub_devs = unsafe {
        set(&sub_dev, kAudioSubDeviceUIDKey, &out_uid);
        NSArray::from_retained_slice(&[sub_dev])
    };
    let tap_uid = unsafe { desc.UUID().UUIDString() };
    let sub_tap = NSMutableDictionary::<NSString, AnyObject>::new();
    let taps = unsafe {
        set(&sub_tap, kAudioSubTapUIDKey, &tap_uid);
        set(
            &sub_tap,
            kAudioSubTapDriftCompensationKey,
            &NSNumber::new_bool(true),
        );
        NSArray::from_retained_slice(&[sub_tap])
    };
    let agg_desc = NSMutableDictionary::<NSString, AnyObject>::new();
    unsafe {
        set(
            &agg_desc,
            kAudioAggregateDeviceNameKey,
            &NSString::from_str("kkm-tap-check"),
        );
        set(
            &agg_desc,
            kAudioAggregateDeviceUIDKey,
            &NSString::from_str("dev.wagomu.kikimimic.tap-check"),
        );
        set(
            &agg_desc,
            kAudioAggregateDeviceIsPrivateKey,
            &NSNumber::new_bool(true),
        );
        set(
            &agg_desc,
            kAudioAggregateDeviceTapAutoStartKey,
            &NSNumber::new_bool(true),
        );
        set(&agg_desc, kAudioAggregateDeviceMainSubDeviceKey, &out_uid);
        set(&agg_desc, kAudioAggregateDeviceSubDeviceListKey, &sub_devs);
        set(&agg_desc, kAudioAggregateDeviceTapListKey, &taps);
    }
    let cf_desc: &CFDictionary = unsafe { &*(Retained::as_ptr(&agg_desc) as *const CFDictionary) };
    let mut agg_id: AudioObjectID = 0;
    let status = unsafe { AudioHardwareCreateAggregateDevice(cf_desc, NonNull::from(&mut agg_id)) };
    check(status, "AudioHardwareCreateAggregateDevice")?;
    println!("aggregate device created: AudioObjectID {agg_id}");
    std::thread::sleep(Duration::from_millis(300)); // let composition settle
    println!(
        "aggregate streams: {} in / {} out",
        stream_count(agg_id, u32::from_be_bytes(*b"inpt")),
        stream_count(agg_id, u32::from_be_bytes(*b"outp")),
    );

    let result = capture(agg_id, &asbd, secs);
    unsafe { AudioHardwareDestroyAggregateDevice(agg_id) };
    result
}

fn capture(agg_id: AudioObjectID, asbd: &AudioStreamBasicDescription, secs: u64) -> Result<()> {
    // 4. Attach an IO block and start IO; the tap's audio arrives as the
    // aggregate device's *input*. The block variant with an explicit queue
    // is used over the fn-pointer IOProc because a nil queue reportedly
    // fails to register on macOS 26.
    let sink = std::sync::Arc::new(Sink {
        samples: Mutex::new(Vec::new()),
        callbacks: AtomicUsize::new(0),
    });
    let block_sink = sink.clone();
    let block = block2::RcBlock::new(
        move |_now: NonNull<AudioTimeStamp>,
              in_data: NonNull<AudioBufferList>,
              _in_time: NonNull<AudioTimeStamp>,
              _out_data: NonNull<AudioBufferList>,
              _out_time: NonNull<AudioTimeStamp>| {
            block_sink.consume(in_data);
        },
    );
    let queue = dispatch2::DispatchQueue::new("kkm-tap-check-io", None);
    let mut proc_id: objc2_core_audio::AudioDeviceIOProcID = None;
    let status = unsafe {
        AudioDeviceCreateIOProcIDWithBlock(
            NonNull::from(&mut proc_id),
            agg_id,
            Some(&queue),
            block2::RcBlock::as_ptr(&block) as _,
        )
    };
    check(status, "AudioDeviceCreateIOProcIDWithBlock")?;
    // Aggregate composition is asynchronous; let it settle before starting
    // IO. The callbacks land on our dispatch queue, so no run loop needed.
    std::thread::sleep(Duration::from_millis(500));
    let status = unsafe { AudioDeviceStart(agg_id, proc_id) };
    check(status, "AudioDeviceStart")?;
    std::thread::sleep(Duration::from_millis(300));
    // False here means the output device is idle — nothing is playing, so
    // the tap has no IO cycle to ride on.
    println!("device running: {}", device_is_running(agg_id));

    println!("capturing system audio for {secs}s — play something...");
    std::thread::sleep(Duration::from_secs(secs));

    unsafe {
        AudioDeviceStop(agg_id, proc_id);
        AudioDeviceDestroyIOProcID(agg_id, proc_id);
    }

    // 5. Verdict: did the callback fire, and was the audio non-silent?
    let callbacks = sink.callbacks.load(Ordering::Relaxed);
    let samples = sink.samples.lock().unwrap();
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let rms = (samples.iter().map(|s| (*s as f64).powi(2)).sum::<f64>()
        / samples.len().max(1) as f64)
        .sqrt();
    let stats = format!(
        "callbacks: {callbacks}, samples: {}, peak: {peak:.4}, rms: {rms:.5}",
        samples.len()
    );
    println!("{stats}");

    let channels = asbd.mChannelsPerFrame.max(1) as u16;
    let spec = hound::WavSpec {
        channels,
        sample_rate: asbd.mSampleRate as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(WAV_PATH, spec).context("create wav")?;
    for s in samples.iter() {
        writer.write_sample(*s)?;
    }
    writer.finalize()?;
    println!("wrote {WAV_PATH}");

    let verdict = if callbacks == 0 {
        "FAIL: the IOProc never fired — was anything playing? An idle output \
         device runs no IO cycle for the tap to ride on."
            .to_string()
    } else if peak < 0.001 {
        "FAIL: the IOProc ran but every sample was zero — macOS is withholding \
         the System Audio Recording permission. Launch from a signed .app \
         bundle via `open` so the app itself is the responsible process."
            .to_string()
    } else {
        format!("PASS: captured non-silent system audio from Rust (peak {peak:.4})")
    };
    println!("{verdict}");
    // `open`-launched runs have nowhere to print, so leave the verdict on disk.
    std::fs::write(
        REPORT_PATH,
        format!("{stats}\nwrote {WAV_PATH}\n{verdict}\n"),
    )
    .context("write report")?;
    anyhow::ensure!(!verdict.starts_with("FAIL"), "{verdict}");
    Ok(())
}

/// Number of streams ('stm#') the device has in the given scope.
fn stream_count(dev: AudioObjectID, scope: u32) -> usize {
    let mut addr = AudioObjectPropertyAddress {
        mSelector: u32::from_be_bytes(*b"stm#"),
        mScope: scope,
        mElement: kAudioObjectPropertyElementMain,
    };
    let mut size: u32 = 0;
    let status = unsafe {
        objc2_core_audio::AudioObjectGetPropertyDataSize(
            dev,
            NonNull::from(&mut addr),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
        )
    };
    if status != 0 {
        return 0;
    }
    size as usize / std::mem::size_of::<AudioObjectID>()
}

/// kAudioDevicePropertyDeviceIsRunning ('goin'): is the device's IO active?
fn device_is_running(dev: AudioObjectID) -> bool {
    let mut addr = AudioObjectPropertyAddress {
        mSelector: u32::from_be_bytes(*b"goin"),
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };
    let mut running: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            dev,
            NonNull::from(&mut addr),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::new(&mut running as *mut _ as *mut c_void).unwrap(),
        )
    };
    status == 0 && running != 0
}

/// The default output device and its UID (the aggregate's clock source).
fn default_output_device() -> Result<(AudioObjectID, Retained<NSString>)> {
    let mut addr = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDefaultOutputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };
    let mut dev: AudioObjectID = 0;
    let mut size = std::mem::size_of::<AudioObjectID>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            kAudioObjectSystemObject as AudioObjectID,
            NonNull::from(&mut addr),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::new(&mut dev as *mut _ as *mut c_void).unwrap(),
        )
    };
    check(status, "get default output device")?;

    let mut addr = AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyDeviceUID,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };
    // CFStringRef, toll-free bridged to NSString; the property returns +1
    // and Retained::from_raw takes over that reference.
    let mut uid: *mut NSString = std::ptr::null_mut();
    let mut size = std::mem::size_of::<*mut NSString>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            dev,
            NonNull::from(&mut addr),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::new(&mut uid as *mut _ as *mut c_void).unwrap(),
        )
    };
    check(status, "get default output device UID")?;
    let uid = unsafe { Retained::from_raw(uid) }.context("default output device UID was null")?;
    Ok((dev, uid))
}

/// NSDictionary insert with a CoreAudio `&CStr` key.
unsafe fn set(
    dict: &NSMutableDictionary<NSString, AnyObject>,
    key: &std::ffi::CStr,
    value: &AnyObject,
) {
    dict.setObject_forKey(value, ProtocolObject::from_ref(&*nsstr(key)));
}
