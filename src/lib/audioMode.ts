export type AudioStream = 'mic' | 'speaker';
export type CaptureMode = AudioStream | 'both';

export function streamsForCaptureMode(mode: CaptureMode): AudioStream[] {
  switch (mode) {
    case 'mic':
      return ['mic'];
    case 'speaker':
      return ['speaker'];
    case 'both':
      return ['mic', 'speaker'];
  }
}

export function captureModeFromStreams(streams: Iterable<AudioStream>): CaptureMode {
  const activeStreams = new Set(streams);

  if (activeStreams.has('mic') && activeStreams.has('speaker')) {
    return 'both';
  }

  if (activeStreams.has('speaker')) {
    return 'speaker';
  }

  return 'mic';
}
