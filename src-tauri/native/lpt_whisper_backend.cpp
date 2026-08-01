#include "lpt_whisper_backend.h"

#include "whisper.h"

#include <cstdlib>
#include <cstring>
#include <new>
#include <sstream>
#include <string>

namespace {

thread_local std::string last_error;

struct LptWhisperBackend {
  whisper_context * context = nullptr;
  std::string language = "auto";
};

void set_error(const std::string & message) {
  last_error = message;
}

char * duplicate_string(const std::string & value) {
  char * copy = static_cast<char *>(std::malloc(value.size() + 1));
  if (copy == nullptr) {
    return nullptr;
  }

  std::memcpy(copy, value.c_str(), value.size() + 1);
  return copy;
}

std::string joined_segments(whisper_context * context) {
  std::string text;
  const int count = whisper_full_n_segments(context);
  for (int index = 0; index < count; index++) {
    const char * segment = whisper_full_get_segment_text(context, index);
    if (segment != nullptr) {
      text += segment;
    }
  }
  return text;
}

std::string detected_language(whisper_context * context, const std::string & fallback) {
  const int language_id = whisper_full_lang_id(context);
  const char * language = language_id >= 0 ? whisper_lang_str(language_id) : nullptr;
  if (language == nullptr || language[0] == '\0') {
    return fallback;
  }
  return language;
}

} // namespace

void * lpt_whisper_backend_create(const char * model_path, const char * language) {
  if (model_path == nullptr || model_path[0] == '\0') {
    set_error("model path is empty");
    return nullptr;
  }

  whisper_context_params context_params = whisper_context_default_params();
  whisper_context * context = whisper_init_from_file_with_params(model_path, context_params);
  if (context == nullptr) {
    std::ostringstream message;
    message << "failed to initialize whisper context for " << model_path;
    set_error(message.str());
    return nullptr;
  }

  LptWhisperBackend * backend = new (std::nothrow) LptWhisperBackend();
  if (backend == nullptr) {
    whisper_free(context);
    set_error("failed to allocate whisper backend");
    return nullptr;
  }

  backend->context = context;
  backend->language = language != nullptr && language[0] != '\0' ? language : "auto";
  return backend;
}

void lpt_whisper_backend_free(void * backend) {
  if (backend == nullptr) {
    return;
  }

  LptWhisperBackend * typed = static_cast<LptWhisperBackend *>(backend);
  if (typed->context != nullptr) {
    whisper_free(typed->context);
  }
  delete typed;
}

int lpt_whisper_backend_transcribe(
    void * backend,
    const float * samples,
    size_t sample_count,
    LptWhisperTranscript * transcript) {
  if (backend == nullptr) {
    set_error("backend is null");
    return -1;
  }
  if (samples == nullptr || sample_count == 0) {
    return 0;
  }
  if (transcript == nullptr) {
    set_error("transcript output is null");
    return -1;
  }

  LptWhisperBackend * typed = static_cast<LptWhisperBackend *>(backend);
  whisper_full_params params = whisper_full_default_params(WHISPER_SAMPLING_GREEDY);
  params.print_progress = false;
  params.print_realtime = false;
  params.print_timestamps = false;
  params.no_timestamps = true;
  params.single_segment = true;
  params.language = typed->language.c_str();

  if (whisper_full(typed->context, params, samples, static_cast<int>(sample_count)) != 0) {
    set_error("whisper_full failed");
    return -1;
  }

  const std::string text = joined_segments(typed->context);
  if (text.empty()) {
    return 0;
  }

  transcript->text = duplicate_string(text);
  transcript->language = duplicate_string(detected_language(typed->context, typed->language));
  transcript->confidence = 0.0;
  transcript->has_confidence = 0;
  if (transcript->text == nullptr || transcript->language == nullptr) {
    lpt_whisper_backend_free_transcript(transcript);
    set_error("failed to allocate transcript strings");
    return -1;
  }

  return 1;
}

void lpt_whisper_backend_free_transcript(LptWhisperTranscript * transcript) {
  if (transcript == nullptr) {
    return;
  }

  std::free(const_cast<char *>(transcript->text));
  std::free(const_cast<char *>(transcript->language));
  transcript->text = nullptr;
  transcript->language = nullptr;
  transcript->confidence = 0.0;
  transcript->has_confidence = 0;
}

const char * lpt_whisper_backend_last_error(void) {
  return last_error.c_str();
}
