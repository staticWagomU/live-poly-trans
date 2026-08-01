#pragma once

#include <stddef.h>

#if defined(_WIN32)
#define LPT_WHISPER_API __declspec(dllexport)
#else
#define LPT_WHISPER_API __attribute__((visibility("default")))
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef struct LptWhisperTranscript {
  const char * text;
  const char * language;
  double confidence;
  unsigned char has_confidence;
} LptWhisperTranscript;

LPT_WHISPER_API void * lpt_whisper_backend_create(const char * model_path, const char * language);
LPT_WHISPER_API void lpt_whisper_backend_free(void * backend);
LPT_WHISPER_API int lpt_whisper_backend_transcribe(
    void * backend,
    const float * samples,
    size_t sample_count,
    LptWhisperTranscript * transcript);
LPT_WHISPER_API void lpt_whisper_backend_free_transcript(LptWhisperTranscript * transcript);
LPT_WHISPER_API const char * lpt_whisper_backend_last_error(void);

#ifdef __cplusplus
}
#endif
