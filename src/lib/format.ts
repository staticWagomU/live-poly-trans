/// Turning the backend's milliseconds and directory names into the labels the
/// window shows. Kept out of the components so the rules read as rules.

/// Position in the recording, as an audio player would show it.
export function clock(ms: number): string {
  const total = Math.floor(ms / 1000);
  const mm = String(Math.floor(total / 60) % 60).padStart(2, '0');
  const ss = String(total % 60).padStart(2, '0');
  const hours = Math.floor(total / 3600);
  return hours ? `${hours}:${mm}:${ss}` : `${mm}:${ss}`;
}

/// How long a session ran, at the coarseness a list wants: minutes for
/// anything that is a meeting, seconds only for the ones too short to round.
export function durationLabel(ms: number): string {
  const minutes = Math.round(ms / 60_000);
  if (minutes < 1) return `${Math.max(1, Math.round(ms / 1000))}秒`;
  if (minutes < 60) return `${minutes}分`;
  const rest = minutes % 60;
  return rest ? `${Math.floor(minutes / 60)}時間${rest}分` : `${Math.floor(minutes / 60)}時間`;
}

export function timeOfDay(ms: number): string {
  const d = new Date(ms);
  return `${d.getHours()}:${String(d.getMinutes()).padStart(2, '0')}`;
}

/// The heading a recording is filed under. Today and yesterday get their
/// names because that is how a recording made this week is looked for;
/// everything older is just its date.
export function dayGroup(startedAtMs: number | null, now = new Date()): string {
  if (startedAtMs === null) return 'その他';
  const midnight = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const days = Math.floor((midnight - startOfDay(startedAtMs)) / 86_400_000);
  if (days <= 0) return '今日';
  if (days === 1) return '昨日';
  const d = new Date(startedAtMs);
  return `${d.getFullYear()}/${d.getMonth() + 1}/${d.getDate()}`;
}

function startOfDay(ms: number): number {
  const d = new Date(ms);
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/// The wall clock a session directory is named after (`20260819090503`, with
/// the `-2` a same-second restart gets). Read the same way as the Rust side,
/// so the running session's elapsed time survives a webview reload: it counts
/// from when the recording actually started, not from when this page loaded.
export function sessionStart(name: string | null): number | null {
  const stamp = name?.split('-')[0];
  if (!stamp || !/^\d{14}$/.test(stamp)) return null;
  const [y, mo, d, h, mi, s] = [
    stamp.slice(0, 4),
    stamp.slice(4, 6),
    stamp.slice(6, 8),
    stamp.slice(8, 10),
    stamp.slice(10, 12),
    stamp.slice(12, 14)
  ].map(Number);
  return new Date(y, mo - 1, d, h, mi, s).getTime();
}
