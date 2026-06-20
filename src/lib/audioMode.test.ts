import { describe, expect, it } from 'vitest';
import { captureModeFromStreams, streamsForCaptureMode } from './audioMode';

describe('streamsForCaptureMode', () => {
  it('maps capture modes to native audio streams', () => {
    expect(streamsForCaptureMode('mic')).toEqual(['mic']);
    expect(streamsForCaptureMode('both')).toEqual(['mic', 'speaker']);
    expect(streamsForCaptureMode('speaker')).toEqual(['speaker']);
  });
});

describe('captureModeFromStreams', () => {
  it('maps active native streams back to the selected capture mode', () => {
    expect(captureModeFromStreams(['mic'])).toBe('mic');
    expect(captureModeFromStreams(['mic', 'speaker'])).toBe('both');
    expect(captureModeFromStreams(['speaker'])).toBe('speaker');
  });
});
