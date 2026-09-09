import { expect, it } from 'vitest';
import { parseChangeRevision } from './changeRevision';

it('accepts opaque strings and safe numeric revisions', () => {
  expect(parseChangeRevision('{"revision":"epoch:42"}')).toBe('epoch:42');
  expect(parseChangeRevision('{"revision":0}')).toBe('0');
  expect(parseChangeRevision('{"revision":42}')).toBe('42');
});

it('ignores malformed and unsupported revisions without throwing', () => {
  for (const data of ['{', 'null', '[]', '{}', '{"revision":null}',
    '{"revision":{}}', '{"revision":true}', '{"revision":""}',
    '{"revision":"  "}', '{"revision":-1}', '{"revision":0.5}',
    '{"revision":9007199254740993}']) {
    expect(parseChangeRevision(data)).toBeNull();
  }
});
