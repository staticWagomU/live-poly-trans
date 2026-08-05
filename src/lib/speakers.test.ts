import { describe, expect, it } from 'vitest';
import {
  SPEAKER_PALETTE,
  defaultStreamSpeakerLabel,
  diarizedSpeakerLabel,
  speakerColor,
  transcriptItemSpeakerId
} from './speakers';

describe('speakerColor', () => {
  it('maps indices onto the palette in order', () => {
    expect(speakerColor(0)).toBe(SPEAKER_PALETTE[0]);
    expect(speakerColor(5)).toBe(SPEAKER_PALETTE[5]);
  });

  it('wraps around when the index exceeds the palette', () => {
    expect(speakerColor(SPEAKER_PALETTE.length)).toBe(SPEAKER_PALETTE[0]);
    expect(speakerColor(SPEAKER_PALETTE.length + 2)).toBe(SPEAKER_PALETTE[2]);
  });
});

describe('transcriptItemSpeakerId', () => {
  it('uses speaker-N for diarized items', () => {
    expect(transcriptItemSpeakerId({ speakerIndex: 0, stream: 'unknown' })).toBe('speaker-0');
    expect(transcriptItemSpeakerId({ speakerIndex: 3, stream: 'mic' })).toBe('speaker-3');
  });

  it('falls back to the stream name without a speaker index', () => {
    expect(transcriptItemSpeakerId({ stream: 'mic' })).toBe('mic');
    expect(transcriptItemSpeakerId({ stream: 'speaker' })).toBe('speaker');
  });
});

describe('defaultStreamSpeakerLabel', () => {
  it('labels the mic stream as Speaker A', () => {
    expect(defaultStreamSpeakerLabel('mic')).toBe('Speaker A');
  });

  it('labels any other stream as Speaker B', () => {
    expect(defaultStreamSpeakerLabel('speaker')).toBe('Speaker B');
    expect(defaultStreamSpeakerLabel('unknown')).toBe('Speaker B');
  });
});

describe('diarizedSpeakerLabel', () => {
  it('turns the 0-based index into a 1-based 話者 label', () => {
    expect(diarizedSpeakerLabel(0)).toBe('話者1');
    expect(diarizedSpeakerLabel(6)).toBe('話者7');
  });
});
