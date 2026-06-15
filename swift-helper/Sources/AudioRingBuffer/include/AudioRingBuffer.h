#ifndef LIVE_POLY_TRANS_AUDIO_RING_BUFFER_H
#define LIVE_POLY_TRANS_AUDIO_RING_BUFFER_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct LPTRingBuffer LPTRingBuffer;

LPTRingBuffer* LPTAudioRingBufferCreate(size_t capacity);
void LPTAudioRingBufferDestroy(LPTRingBuffer* buffer);
size_t LPTAudioRingBufferCapacity(const LPTRingBuffer* buffer);
size_t LPTAudioRingBufferAvailable(const LPTRingBuffer* buffer);
size_t LPTAudioRingBufferWrite(LPTRingBuffer* buffer, const void* data, size_t byteCount);
void LPTAudioRingBufferClear(LPTRingBuffer* buffer);

#ifdef __cplusplus
}
#endif

#endif
