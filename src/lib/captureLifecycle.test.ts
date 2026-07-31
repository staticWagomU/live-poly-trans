import { describe, expect, it, vi } from 'vitest';
import { completeCaptureStop } from './captureLifecycle';

describe('completeCaptureStop', () => {
  it('finalizes the recording after capture sessions stop', async () => {
    const stopSessions = vi.fn().mockResolvedValue(undefined);
    const finalizeRecording = vi.fn().mockResolvedValue(undefined);

    const errors = await completeCaptureStop(stopSessions, finalizeRecording);

    expect(stopSessions).toHaveBeenCalledOnce();
    expect(finalizeRecording).toHaveBeenCalledOnce();
    expect(stopSessions.mock.invocationCallOrder[0]).toBeLessThan(
      finalizeRecording.mock.invocationCallOrder[0]
    );
    expect(errors).toEqual([]);
  });

  it('still finalizes the recording when stopping sessions fails', async () => {
    const stopError = new Error('stop failed');
    const finalizeRecording = vi.fn().mockResolvedValue(undefined);

    const errors = await completeCaptureStop(
      vi.fn().mockRejectedValue(stopError),
      finalizeRecording
    );

    expect(finalizeRecording).toHaveBeenCalledOnce();
    expect(errors).toEqual([stopError]);
  });

  it('reports both stop and finalization failures', async () => {
    const stopError = new Error('stop failed');
    const finalizeError = new Error('finalize failed');

    const errors = await completeCaptureStop(
      vi.fn().mockRejectedValue(stopError),
      vi.fn().mockRejectedValue(finalizeError)
    );

    expect(errors).toEqual([stopError, finalizeError]);
  });
});
