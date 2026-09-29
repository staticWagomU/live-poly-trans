/// Model files the settings screen lists. The backend owns the paths and the
/// bytes; this is only how the window talks about them.

export type ModelState = 'missing' | 'ready' | 'downloading';

export type ModelFileStatus = {
  id: string;
  fileName: string;
  state: ModelState;
  path: string | null;
  bytes: number | null;
  totalBytes: number | null;
  canDownload: boolean;
  message: string | null;
};

export type ModelDownloadEvent = {
  id: string;
  receivedBytes: number;
  totalBytes: number | null;
};

const COPY: Record<string, { title: string; detail: string }> = {
  asr: { title: '音声認識', detail: 'Whisper large-v3-turbo' },
  vad: { title: '発話検出', detail: 'Silero VAD' },
  translator: { title: '翻訳', detail: 'Qwen3-4B Instruct' }
};

export function modelCopy(id: string): { title: string; detail: string } {
  return COPY[id] ?? { title: id, detail: '' };
}

/// Decimal units, the way the file size is usually quoted (834 MB, 2.5 GB).
export function byteLabel(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '0 B';
  const units: [number, string][] = [
    [1_000_000_000, 'GB'],
    [1_000_000, 'MB'],
    [1_000, 'KB']
  ];
  for (const [size, unit] of units) {
    if (bytes >= size) {
      const value = bytes / size;
      const digits = value >= 10 ? 0 : 1;
      return `${value.toFixed(digits)} ${unit}`;
    }
  }
  return `${Math.round(bytes)} B`;
}
