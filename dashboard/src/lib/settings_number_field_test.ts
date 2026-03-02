/**
 * Tests for NumberField integer bounds validation in settings-panel.tsx.
 *
 * Static-analysis tests that read the source and verify the bounds-checking
 * logic is present before the onChange callback is invoked.
 *
 * These tests FAIL initially because parseInt(e.target.value) is forwarded
 * to onChange without any min/max check.
 */

import { describe, test, expect, beforeAll } from "bun:test";
import { readFileSync } from "fs";
import { join } from "path";

const SRC_PATH = join(import.meta.dir, "../components/settings-panel.tsx");
let src: string;
let numberFieldBody: string;

function extractFunctionBody(source: string, name: string): string {
  const marker = `function ${name}(`;
  const markerIdx = source.indexOf(marker);
  if (markerIdx === -1) throw new Error(`Function '${name}' not found in source`);

  // Skip past the parameter list by tracking paren depth
  let i = markerIdx + marker.length;
  let parenDepth = 1;
  while (i < source.length && parenDepth > 0) {
    if (source[i] === "(") parenDepth++;
    else if (source[i] === ")") parenDepth--;
    i++;
  }
  // i is now past the closing ) — find the opening { of the function body
  while (i < source.length && source[i] !== "{") i++;
  const bodyStart = i;

  let braceDepth = 0;
  while (i < source.length) {
    if (source[i] === "{") braceDepth++;
    else if (source[i] === "}") {
      braceDepth--;
      if (braceDepth === 0) return source.slice(bodyStart, i + 1);
    }
    i++;
  }
  throw new Error(`Could not find closing brace for '${name}'`);
}

beforeAll(() => {
  src = readFileSync(SRC_PATH, "utf-8");
  numberFieldBody = extractFunctionBody(src, "NumberField");
});

// ---------------------------------------------------------------------------
// AC1: parseInt is still used to parse the input value
// ---------------------------------------------------------------------------

describe("AC1: parseInt is used to parse the numeric input", () => {
  test("parseInt is called inside NumberField onChange", () => {
    expect(numberFieldBody).toContain("parseInt");
  });
});

// ---------------------------------------------------------------------------
// AC2: NaN guard is present
// ---------------------------------------------------------------------------

describe("AC2: NaN values are rejected before calling onChange", () => {
  test("isNaN check is present in NumberField", () => {
    expect(numberFieldBody).toContain("isNaN");
  });
});

// ---------------------------------------------------------------------------
// AC3: Lower bound (min) is enforced with Math.max
// ---------------------------------------------------------------------------

describe("AC3: Lower bound is enforced", () => {
  test("Math.max is called to enforce the min bound", () => {
    expect(numberFieldBody).toContain("Math.max");
  });

  test("min is referenced in the Math.max call", () => {
    const maxIdx = numberFieldBody.indexOf("Math.max");
    expect(maxIdx).toBeGreaterThan(-1);
    // min must appear within 60 chars of Math.max
    const slice = numberFieldBody.slice(maxIdx, maxIdx + 60);
    expect(slice).toContain("min");
  });
});

// ---------------------------------------------------------------------------
// AC4: Upper bound (max) is enforced with Math.min
// ---------------------------------------------------------------------------

describe("AC4: Upper bound is enforced", () => {
  test("Math.min is called to enforce the max bound", () => {
    expect(numberFieldBody).toContain("Math.min");
  });

  test("max is referenced in the Math.min call", () => {
    const minIdx = numberFieldBody.indexOf("Math.min");
    expect(minIdx).toBeGreaterThan(-1);
    const slice = numberFieldBody.slice(minIdx, minIdx + 60);
    expect(slice).toContain("max");
  });
});

// ---------------------------------------------------------------------------
// AC5: onChange is called only after bounds-checking
// ---------------------------------------------------------------------------

describe("AC5: onChange is called after bounds are applied", () => {
  test("onChange call appears after Math.max in NumberField body", () => {
    const maxIdx = numberFieldBody.indexOf("Math.max");
    const onChangeIdx = numberFieldBody.lastIndexOf("onChange(");
    expect(maxIdx).toBeGreaterThan(-1);
    expect(onChangeIdx).toBeGreaterThan(maxIdx);
  });

  test("onChange call appears after Math.min in NumberField body", () => {
    const minIdx = numberFieldBody.indexOf("Math.min");
    const onChangeIdx = numberFieldBody.lastIndexOf("onChange(");
    expect(minIdx).toBeGreaterThan(-1);
    expect(onChangeIdx).toBeGreaterThan(minIdx);
  });
});

// ---------------------------------------------------------------------------
// AC6: min/max are guarded so they only clamp when defined
// ---------------------------------------------------------------------------

describe("AC6: bounds are applied only when min/max props are defined", () => {
  test("min is checked for undefined before clamping (undefined guard)", () => {
    // Either `min !== undefined` or `min != null` or conditional with `??`
    const hasMinGuard =
      numberFieldBody.includes("min !== undefined") ||
      numberFieldBody.includes("min != null") ||
      numberFieldBody.includes("min ??") ||
      numberFieldBody.includes("?? v") ||
      numberFieldBody.includes("min,");
    expect(hasMinGuard).toBe(true);
  });

  test("max is checked for undefined before clamping (undefined guard)", () => {
    const hasMaxGuard =
      numberFieldBody.includes("max !== undefined") ||
      numberFieldBody.includes("max != null") ||
      numberFieldBody.includes("max ??") ||
      numberFieldBody.includes("?? v") ||
      numberFieldBody.includes("max,");
    expect(hasMaxGuard).toBe(true);
  });
});
