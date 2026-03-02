/**
 * Tests for chat-panel fetchMessages error handling.
 *
 * Static-analysis tests: read the source of chat-panel.tsx and verify the
 * correct error-handling patterns are present. Uses only Bun built-ins.
 */

import { describe, test, expect, beforeAll } from "bun:test";
import { readFileSync } from "fs";
import { join } from "path";

const SRC_PATH = join(import.meta.dir, "..", "components", "chat-panel.tsx");
let src: string;

/**
 * Extracts the text of the useCallback body assigned to `fetchMessages`.
 * Finds `const fetchMessages = useCallback(` and tracks brace depth to get
 * the full callback source.
 */
function extractFetchMessagesBody(source: string): string {
  const marker = "const fetchMessages = useCallback(";
  const start = source.indexOf(marker);
  if (start === -1) throw new Error("fetchMessages useCallback not found");

  let depth = 0;
  let opened = false;
  let i = start;
  while (i < source.length) {
    if (source[i] === "{") { depth++; opened = true; }
    else if (source[i] === "}" && opened) {
      depth--;
      if (depth === 0) return source.slice(start, i + 1);
    }
    i++;
  }
  throw new Error("Could not find closing brace for fetchMessages");
}

beforeAll(() => {
  src = readFileSync(SRC_PATH, "utf-8");
});

// ---------------------------------------------------------------------------
// AC1 — no empty catch in fetchMessages
// ---------------------------------------------------------------------------

describe("AC1: fetchMessages catch is not empty", () => {
  test("empty .catch(() => {}) is absent from fetchMessages", () => {
    const body = extractFetchMessagesBody(src);
    const emptycatch = /\.catch\s*\(\s*\(\s*\)\s*=>\s*\{\s*\}\s*\)/;
    expect(emptycatch.test(body)).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// AC2 — console.error is called in fetchMessages catch
// ---------------------------------------------------------------------------

describe("AC2: console.error is called inside fetchMessages catch", () => {
  test("console.error is present in fetchMessages body", () => {
    const body = extractFetchMessagesBody(src);
    expect(/console\.error/.test(body)).toBe(true);
  });

  test("console.error receives the error argument", () => {
    const body = extractFetchMessagesBody(src);
    const pattern = /console\.error\s*\([^)]+err/;
    expect(pattern.test(body)).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// AC3 — fetchError state is declared in ChatPanel
// ---------------------------------------------------------------------------

describe("AC3: fetchError state variable is declared", () => {
  test("useState<string | null> for fetchError is declared", () => {
    const pattern = /const\s*\[\s*fetchError\s*,\s*setFetchError\s*\]\s*=\s*useState/;
    expect(pattern.test(src)).toBe(true);
  });

  test("setFetchError is called with a non-null string on failure", () => {
    const body = extractFetchMessagesBody(src);
    const pattern = /setFetchError\s*\(\s*["'][^"']+["']\s*\)/;
    expect(pattern.test(body)).toBe(true);
  });

  test("setFetchError(null) is called on success to clear the error", () => {
    const body = extractFetchMessagesBody(src);
    const pattern = /setFetchError\s*\(\s*null\s*\)/;
    expect(pattern.test(body)).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// AC4 — fetchError is rendered in the JSX
// ---------------------------------------------------------------------------

describe("AC4: fetchError is rendered visibly in the component JSX", () => {
  test("fetchError is referenced in JSX conditional render", () => {
    const pattern = /\{fetchError\s*&&/;
    expect(pattern.test(src)).toBe(true);
  });

  test("fetchError value is interpolated inside a JSX element", () => {
    const pattern = /\{fetchError\}/;
    expect(pattern.test(src)).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// AC5 — aborted requests do not trigger error state
// ---------------------------------------------------------------------------

describe("AC5: aborted requests are ignored in the catch handler", () => {
  test("catch checks controller.signal.aborted before setting error", () => {
    const body = extractFetchMessagesBody(src);
    const catchIdx = body.lastIndexOf(".catch(");
    expect(catchIdx).toBeGreaterThan(-1);
    const catchBody = body.slice(catchIdx);
    expect(/controller\.signal\.aborted/.test(catchBody)).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// AC6 — poll fallback and fetchThreads also log errors
// ---------------------------------------------------------------------------

describe("AC6: poll fallback and fetchThreads log errors", () => {
  test("poll interval fetch catch calls console.error", () => {
    // Find the setInterval block and check its catch
    const intervalIdx = src.indexOf("setInterval(");
    expect(intervalIdx).toBeGreaterThan(-1);
    const intervalBlock = src.slice(intervalIdx, src.indexOf("}, 3000)") + 10);
    expect(/console\.error/.test(intervalBlock)).toBe(true);
  });

  test("fetchThreads catch calls console.error", () => {
    const marker = "const fetchThreads = useCallback(";
    const start = src.indexOf(marker);
    expect(start).toBeGreaterThan(-1);
    // Find end of useCallback — locate next occurrence of "}, [" after start
    const end = src.indexOf("}, []);", start);
    const body = src.slice(start, end + 7);
    expect(/console\.error/.test(body)).toBe(true);
  });
});
