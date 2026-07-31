export type CaptureStopOperation = () => Promise<unknown>;

export async function completeCaptureStop(
  stopSessions: CaptureStopOperation,
  finalizeRecording: CaptureStopOperation
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
