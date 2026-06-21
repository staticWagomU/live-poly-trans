import AVFAudio
import Foundation

public struct AudioSignalLevel: Equatable {
  public let sampleCount: Int
  public let rms: Double
  public let peak: Double

  public init(sampleCount: Int, rms: Double, peak: Double) {
    self.sampleCount = sampleCount
    self.rms = rms
    self.peak = peak
  }

  public var description: String {
    guard sampleCount > 0 else {
      return "samples=0 rms=0.000000 peak=0.000000"
    }

    return String(format: "samples=%d rms=%.6f peak=%.6f", sampleCount, rms, peak)
  }
}

public func audioSignalLevel(_ buffer: AVAudioPCMBuffer) -> AudioSignalLevel {
  audioSignalLevel(
    buffers: UnsafeMutableAudioBufferListPointer(buffer.mutableAudioBufferList),
    commonFormat: buffer.format.commonFormat
  )
}

public func audioSignalLevel(
  bufferList: UnsafePointer<AudioBufferList>,
  format: AVAudioFormat
) -> AudioSignalLevel {
  audioSignalLevel(
    buffers: UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: bufferList)),
    commonFormat: format.commonFormat
  )
}

public func audioSignalLevel(
  bufferList: UnsafeMutablePointer<AudioBufferList>,
  format: AVAudioFormat
) -> AudioSignalLevel {
  audioSignalLevel(
    buffers: UnsafeMutableAudioBufferListPointer(bufferList),
    commonFormat: format.commonFormat
  )
}

public func audioSignalLevel(
  buffers: UnsafeMutableAudioBufferListPointer,
  commonFormat: AVAudioCommonFormat
) -> AudioSignalLevel {
  var sampleCount = 0
  var sumSquares = 0.0
  var peak = 0.0

  for audioBuffer in buffers {
    guard let data = audioBuffer.mData else {
      continue
    }

    switch commonFormat {
    case .pcmFormatFloat32:
      let samples = data.assumingMemoryBound(to: Float.self)
      let count = Int(audioBuffer.mDataByteSize) / MemoryLayout<Float>.size
      for index in 0..<count {
        let value = Double(samples[index])
        sampleCount += 1
        sumSquares += value * value
        peak = max(peak, abs(value))
      }
    case .pcmFormatFloat64:
      let samples = data.assumingMemoryBound(to: Double.self)
      let count = Int(audioBuffer.mDataByteSize) / MemoryLayout<Double>.size
      for index in 0..<count {
        let value = samples[index]
        sampleCount += 1
        sumSquares += value * value
        peak = max(peak, abs(value))
      }
    case .pcmFormatInt16:
      let samples = data.assumingMemoryBound(to: Int16.self)
      let count = Int(audioBuffer.mDataByteSize) / MemoryLayout<Int16>.size
      for index in 0..<count {
        let value = Double(samples[index]) / Double(Int16.max)
        sampleCount += 1
        sumSquares += value * value
        peak = max(peak, abs(value))
      }
    case .pcmFormatInt32:
      let samples = data.assumingMemoryBound(to: Int32.self)
      let count = Int(audioBuffer.mDataByteSize) / MemoryLayout<Int32>.size
      for index in 0..<count {
        let value = Double(samples[index]) / Double(Int32.max)
        sampleCount += 1
        sumSquares += value * value
        peak = max(peak, abs(value))
      }
    default:
      return AudioSignalLevel(sampleCount: 0, rms: 0, peak: 0)
    }
  }

  guard sampleCount > 0 else {
    return AudioSignalLevel(sampleCount: 0, rms: 0, peak: 0)
  }

  return AudioSignalLevel(
    sampleCount: sampleCount,
    rms: sqrt(sumSquares / Double(sampleCount)),
    peak: peak
  )
}
