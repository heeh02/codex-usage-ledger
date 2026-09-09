export function sourcePolicyCopy(policy?: string | null) {
  if (policy === 'request_union_v2') return { title: 'source.union_title', description: 'source.union_description' } as const;
  if (policy === undefined || policy === null || policy === 'max_thread_day_v1') return { title: 'components.explorer.only_one_record_source_is_selected_per', description: 'components.explorer.reconstructed_history_is_selected_when_it_is' } as const;
  return { title: 'source.unknown_title', description: 'source.unknown_description' } as const;
}
