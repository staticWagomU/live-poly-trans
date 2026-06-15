#include "AudioRingBuffer.h"

#include <algorithm>
#include <cstdint>
#include <cstring>
#include <mutex>
#include <vector>

struct LPTRingBuffer {
  explicit LPTRingBuffer(size_t capacity) : storage(std::max<size_t>(capacity, 1)) {}

  std::vector<std::uint8_t> storage;
  size_t writeIndex = 0;
  size_t available = 0;
  mutable std::mutex mutex;
};

LPTRingBuffer* LPTAudioRingBufferCreate(size_t capacity) {
  return new LPTRingBuffer(capacity);
}

void LPTAudioRingBufferDestroy(LPTRingBuffer* buffer) {
  delete buffer;
}

size_t LPTAudioRingBufferCapacity(const LPTRingBuffer* buffer) {
  if (buffer == nullptr) {
    return 0;
  }

  std::lock_guard<std::mutex> lock(buffer->mutex);
  return buffer->storage.size();
}

size_t LPTAudioRingBufferAvailable(const LPTRingBuffer* buffer) {
  if (buffer == nullptr) {
    return 0;
  }

  std::lock_guard<std::mutex> lock(buffer->mutex);
  return buffer->available;
}

size_t LPTAudioRingBufferWrite(LPTRingBuffer* buffer, const void* data, size_t byteCount) {
  if (buffer == nullptr || data == nullptr || byteCount == 0) {
    return 0;
  }

  std::lock_guard<std::mutex> lock(buffer->mutex);
  const auto* bytes = static_cast<const std::uint8_t*>(data);
  const size_t capacity = buffer->storage.size();
  const size_t bytesToWrite = std::min(byteCount, capacity);
  const size_t sourceOffset = byteCount - bytesToWrite;

  for (size_t index = 0; index < bytesToWrite; ++index) {
    buffer->storage[buffer->writeIndex] = bytes[sourceOffset + index];
    buffer->writeIndex = (buffer->writeIndex + 1) % capacity;
  }

  buffer->available = std::min(capacity, buffer->available + bytesToWrite);
  return bytesToWrite;
}

void LPTAudioRingBufferClear(LPTRingBuffer* buffer) {
  if (buffer == nullptr) {
    return;
  }

  std::lock_guard<std::mutex> lock(buffer->mutex);
  buffer->writeIndex = 0;
  buffer->available = 0;
}
