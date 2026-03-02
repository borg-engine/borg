import { describe, test, expect } from "bun:test";

// Extract the validation logic directly for unit testing
const DATE_RE = /^\d{4}-\d{2}-\d{2}$/;

function validateDate(value, name) {
  if (!value || !DATE_RE.test(value)) {
    throw new Error(`${name} must be a valid date in YYYY-MM-DD format (got: ${JSON.stringify(value)})`);
  }
}

describe("validateDate", () => {
  test("accepts valid YYYY-MM-DD dates", () => {
    expect(() => validateDate("2024-01-01", "start_date")).not.toThrow();
    expect(() => validateDate("2024-12-31", "end_date")).not.toThrow();
    expect(() => validateDate("2000-06-15", "start_date")).not.toThrow();
  });

  test("rejects missing value", () => {
    expect(() => validateDate(undefined, "start_date")).toThrow("start_date must be a valid date in YYYY-MM-DD format");
    expect(() => validateDate(null, "end_date")).toThrow("end_date must be a valid date in YYYY-MM-DD format");
    expect(() => validateDate("", "start_date")).toThrow("start_date must be a valid date in YYYY-MM-DD format");
  });

  test("rejects wrong format", () => {
    expect(() => validateDate("01/01/2024", "start_date")).toThrow("start_date must be a valid date in YYYY-MM-DD format");
    expect(() => validateDate("2024/01/01", "end_date")).toThrow("end_date must be a valid date in YYYY-MM-DD format");
    expect(() => validateDate("Jan 1, 2024", "start_date")).toThrow();
    expect(() => validateDate("20240101", "end_date")).toThrow();
    expect(() => validateDate("2024-1-1", "start_date")).toThrow();
  });

  test("includes the bad value in error message", () => {
    expect(() => validateDate("bad-date", "start_date")).toThrow('"bad-date"');
  });

  test("includes the field name in error message", () => {
    expect(() => validateDate("nope", "end_date")).toThrow("end_date must be a valid date");
  });
});
