import { test, expect } from 'bun:test';
import { appendChunk, STDOUT_BUF_MAX } from './stream-buf.js';

test('splits complete lines', () => {
  const { buf, lines } = appendChunk('', 'hello\nworld\n');
  expect(lines).toEqual(['hello', 'world']);
  expect(buf).toBe('');
});

test('accumulates partial lines across chunks', () => {
  const { buf: b1, lines: l1 } = appendChunk('', 'hel');
  expect(l1).toEqual([]);
  expect(b1).toBe('hel');

  const { buf: b2, lines: l2 } = appendChunk(b1, 'lo\n');
  expect(l2).toEqual(['hello']);
  expect(b2).toBe('');
});

test('returns null lines when chunk causes overflow', () => {
  const big = 'x'.repeat(STDOUT_BUF_MAX + 1);
  const { buf, lines } = appendChunk('', big);
  expect(lines).toBeNull();
  expect(buf).toBe('');
});

test('returns null lines when accumulated buf + chunk causes overflow', () => {
  const half = 'x'.repeat(Math.floor(STDOUT_BUF_MAX / 2) + 1);
  const { buf: b1 } = appendChunk('', half);
  const { buf, lines } = appendChunk(b1, half);
  expect(lines).toBeNull();
  expect(buf).toBe('');
});

test('resets buffer to empty on overflow so subsequent data is processed normally', () => {
  const big = 'x'.repeat(STDOUT_BUF_MAX + 1);
  const { buf: afterOverflow } = appendChunk('', big);
  expect(afterOverflow).toBe('');

  const { buf, lines } = appendChunk(afterOverflow, 'ok\n');
  expect(lines).toEqual(['ok']);
  expect(buf).toBe('');
});

test('does not overflow when exactly at max size', () => {
  const exact = 'x'.repeat(STDOUT_BUF_MAX);
  const { lines } = appendChunk('', exact);
  expect(lines).not.toBeNull();
});

test('overflows when one byte over max size', () => {
  const over = 'x'.repeat(STDOUT_BUF_MAX + 1);
  const { lines } = appendChunk('', over);
  expect(lines).toBeNull();
});

test('preserves trailing partial line in buf', () => {
  const { buf, lines } = appendChunk('', 'line1\npartial');
  expect(lines).toEqual(['line1']);
  expect(buf).toBe('partial');
});

test('empty chunk returns empty lines array', () => {
  const { buf, lines } = appendChunk('existing', '');
  expect(lines).toEqual([]);
  expect(buf).toBe('existing');
});
