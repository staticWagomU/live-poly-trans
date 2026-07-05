export type SpeechModelInfo = {
  fileName: string;
  path: string;
  sizeBytes: number;
};

export type SpeechModelsPayload = {
  models: SpeechModelInfo[];
  cliAvailable: boolean;
};

export type SpeechModelSelection =
  | { engine: 'builtin' }
  | { engine: 'whisper'; modelPath: string };

const BUILTIN_PREFERENCE = 'builtin';

export function speechModelLabel(fileName: string): string {
  return fileName.replace(/^ggml-/, '').replace(/\.bin$/, '');
}

export function formatModelSize(sizeBytes: number): string {
  const gigabyte = 1024 ** 3;
  if (sizeBytes >= gigabyte) {
    return `${(sizeBytes / gigabyte).toFixed(1)} GB`;
  }

  return `${Math.round(sizeBytes / 1024 ** 2)} MB`;
}

export function parseSpeechModelPreference(stored: string | null): SpeechModelSelection {
  if (!stored || stored === BUILTIN_PREFERENCE) {
    return { engine: 'builtin' };
  }

  return { engine: 'whisper', modelPath: stored };
}

export function speechModelPreferenceValue(selection: SpeechModelSelection): string {
  return selection.engine === 'builtin' ? BUILTIN_PREFERENCE : selection.modelPath;
}

// A stored whisper selection can go stale (superwhisper removed the model,
// whisper-cpp uninstalled); resolving against the live payload keeps the
// app startable instead of failing every session.
export function resolveSpeechModelSelection(
  selection: SpeechModelSelection,
  models: SpeechModelInfo[],
  cliAvailable: boolean
): SpeechModelSelection {
  if (selection.engine === 'builtin') {
    return selection;
  }

  const available = cliAvailable && models.some((model) => model.path === selection.modelPath);
  return available ? selection : { engine: 'builtin' };
}

export function streamEnginePayload(selection: SpeechModelSelection): {
  engine: 'builtin' | 'whisper';
  whisperModel: string | null;
} {
  return selection.engine === 'builtin'
    ? { engine: 'builtin', whisperModel: null }
    : { engine: 'whisper', whisperModel: selection.modelPath };
}
