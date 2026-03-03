// Stdout line-buffer helper for agent session output.
export const STDOUT_BUF_MAX = 1024 * 1024; // 1 MB

/**
 * Append chunk to buf and split on newlines.
 * Returns { buf, lines } where lines is the array of complete lines found.
 * If the combined length exceeds maxSize, returns { buf: '', lines: null }
 * to signal overflow — the caller should emit a warning and discard the buffer.
 */
export function appendChunk(buf, chunk, maxSize = STDOUT_BUF_MAX) {
  const next = buf + chunk;
  if (next.length > maxSize) return { buf: '', lines: null };
  const parts = next.split('\n');
  return { buf: parts.pop(), lines: parts };
}
