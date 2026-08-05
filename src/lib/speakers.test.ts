import { describe, expect, it } from 'vitest';
import {
  SPEAKER_PALETTE,
  applySpeakerRename,
  defaultStreamSpeakerLabel,
  diarizedSpeakerLabel,
  resolveSpeakerColor,
  resolveSpeakerName,
  speakerColor,
  speakerStats,
  transcriptItemSpeakerId
} from './speakers';
import type { RecordingTranscriptItem } from './recordings';

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

describe('resolveSpeakerName', () => {
  it('returns the custom name from the speakers map', () => {
    const entry = { speakerId: 'speaker-0', speakerLabel: '話者1' };
    expect(resolveSpeakerName(entry, { 'speaker-0': { name: '田中さん' } })).toBe('田中さん');
  });

  it('falls back to the entry label when the map has no entry for the id', () => {
    const entry = { speakerId: 'speaker-1', speakerLabel: '話者2' };
    expect(resolveSpeakerName(entry, { 'speaker-0': { name: '田中さん' } })).toBe('話者2');
  });

  it('falls back to the entry label when the map is missing', () => {
    const entry = { speakerId: 'mic', speakerLabel: 'Speaker A' };
    expect(resolveSpeakerName(entry, undefined)).toBe('Speaker A');
    expect(resolveSpeakerName(entry, null)).toBe('Speaker A');
  });

  it('falls back to the entry label when the custom name is blank', () => {
    const entry = { speakerId: 'speaker-0', speakerLabel: '話者1' };
    expect(resolveSpeakerName(entry, { 'speaker-0': { name: '' } })).toBe('話者1');
    expect(resolveSpeakerName(entry, { 'speaker-0': { name: '   ' } })).toBe('話者1');
  });

  it('falls back to the entry label when the entry has no speaker id', () => {
    const entry = { speakerLabel: '話者' };
    expect(resolveSpeakerName(entry, { 'speaker-0': { name: '田中さん' } })).toBe('話者');
  });
});

describe('resolveSpeakerColor', () => {
  it('prefers the custom color from the speakers map', () => {
    const entry = { speakerId: 'speaker-0', speakerIndex: 0 };
    expect(resolveSpeakerColor(entry, { 'speaker-0': { name: '田中さん', color: '#123456' } })).toBe(
      '#123456'
    );
  });

  it('falls back to the palette color for diarized entries', () => {
    const entry = { speakerId: 'speaker-2', speakerIndex: 2 };
    expect(resolveSpeakerColor(entry, { 'speaker-2': { name: '田中さん' } })).toBe(speakerColor(2));
    expect(resolveSpeakerColor(entry, undefined)).toBe(speakerColor(2));
  });

  it('returns undefined for stream entries without a custom color', () => {
    expect(resolveSpeakerColor({ speakerId: 'mic' }, null)).toBeUndefined();
    expect(resolveSpeakerColor({ speakerId: 'mic' }, { mic: { name: '自分' } })).toBeUndefined();
  });
});

describe('speakerStats', () => {
  const item = (
    overrides: Partial<RecordingTranscriptItem> & Pick<RecordingTranscriptItem, 'stream'>
  ): RecordingTranscriptItem => ({
    key: 'k',
    startMs: 0,
    speakerLabel: 'Speaker A',
    language: 'ja',
    text: 'hello',
    translation: null,
    ...overrides
  });

  it('lists unique speakers in first-appearance order with counts', () => {
    const items = [
      item({ stream: 'unknown', speakerIndex: 1, speakerLabel: '話者2' }),
      item({ stream: 'unknown', speakerIndex: 0, speakerLabel: '話者1' }),
      item({ stream: 'unknown', speakerIndex: 1, speakerLabel: '話者2' }),
      item({ stream: 'unknown', speakerIndex: 1, speakerLabel: '話者2' })
    ];

    expect(speakerStats(items)).toEqual([
      { id: 'speaker-1', label: '話者2', count: 3, speakerIndex: 1 },
      { id: 'speaker-0', label: '話者1', count: 1, speakerIndex: 0 }
    ]);
  });

  it('keys live-stream items by their stream name without a speaker index', () => {
    const items = [
      item({ stream: 'mic', speakerLabel: 'Speaker A' }),
      item({ stream: 'speaker', speakerLabel: 'Speaker B' }),
      item({ stream: 'mic', speakerLabel: 'Speaker A' })
    ];

    expect(speakerStats(items)).toEqual([
      { id: 'mic', label: 'Speaker A', count: 2 },
      { id: 'speaker', label: 'Speaker B', count: 1 }
    ]);
  });

  it('returns an empty list for an empty transcript', () => {
    expect(speakerStats([])).toEqual([]);
  });
});

describe('applySpeakerRename', () => {
  it('adds a trimmed custom name to an empty map', () => {
    expect(applySpeakerRename(null, 'speaker-0', '  田中 ', '話者1')).toEqual({
      'speaker-0': { name: '田中' }
    });
    expect(applySpeakerRename(undefined, 'mic', '自分', 'Speaker A')).toEqual({
      mic: { name: '自分' }
    });
  });

  it('overwrites the name while preserving the entry color', () => {
    const speakers = { 'speaker-0': { name: '田中', color: '#123456' } };
    expect(applySpeakerRename(speakers, 'speaker-0', '佐藤', '話者1')).toEqual({
      'speaker-0': { name: '佐藤', color: '#123456' }
    });
  });

  it('keeps unrelated entries untouched', () => {
    const speakers = { 'speaker-1': { name: 'Sarah' } };
    expect(applySpeakerRename(speakers, 'speaker-0', '田中', '話者1')).toEqual({
      'speaker-0': { name: '田中' },
      'speaker-1': { name: 'Sarah' }
    });
  });

  it('removes the entry when the trimmed name is empty', () => {
    const speakers = {
      'speaker-0': { name: '田中' },
      'speaker-1': { name: 'Sarah' }
    };
    expect(applySpeakerRename(speakers, 'speaker-0', '   ', '話者1')).toEqual({
      'speaker-1': { name: 'Sarah' }
    });
  });

  it('removes the entry when the name equals the default label', () => {
    const speakers = {
      'speaker-0': { name: '田中' },
      'speaker-1': { name: 'Sarah' }
    };
    expect(applySpeakerRename(speakers, 'speaker-0', ' 話者1 ', '話者1')).toEqual({
      'speaker-1': { name: 'Sarah' }
    });
  });

  it('returns null when the map becomes empty', () => {
    expect(applySpeakerRename({ 'speaker-0': { name: '田中' } }, 'speaker-0', '', '話者1')).toBeNull();
    expect(applySpeakerRename(null, 'speaker-0', '話者1', '話者1')).toBeNull();
  });

  it('does not mutate the input map', () => {
    const speakers = { 'speaker-0': { name: '田中' } };
    applySpeakerRename(speakers, 'speaker-0', '佐藤', '話者1');
    applySpeakerRename(speakers, 'speaker-0', '', '話者1');
    expect(speakers).toEqual({ 'speaker-0': { name: '田中' } });
  });
});
