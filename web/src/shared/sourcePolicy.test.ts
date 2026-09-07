import { describe, expect, it } from 'vitest';
import { sourcePolicyCopy } from './sourcePolicy';

describe('source policy presentation', () => {
  it('uses the explicit server policy without inferring from account or quantities', () => {
    expect(sourcePolicyCopy('request_union_v2').title).toBe('source.union_title');
    expect(sourcePolicyCopy().title).toBe('components.explorer.only_one_record_source_is_selected_per');
    expect(sourcePolicyCopy('future-policy').title).toBe('source.unknown_title');
  });
});
