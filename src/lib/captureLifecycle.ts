export type CaptureStopOperation = () => Promise<unknown>;

export async function completeCaptureStop(
  stopSessions: CaptureStopOperation,
  // In the always-on model pausing capture no longer owns a recording to
  // finalize; callers that still do pass their own finalizer.
  finalizeRecording: CaptureStopOperation = async () => {}
): Promise<unknown[]> {
  const errors: unknown[] = [];

  try {
    await stopSessions();
  } catch (error) {
    errors.push(error);
  }

  try {
    await finalizeRecording();
  } catch (error) {
    errors.push(error);
  }

  return errors;
}
