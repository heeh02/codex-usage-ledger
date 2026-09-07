import { useI18n } from '../i18n';

export function ModelUsageLabel({ models, catalogModel }: { models?: Array<string | null> | null; catalogModel?: string | null }) {
  const { t } = useI18n();
  const known = models?.filter((model): model is string => model !== null) ?? [];
  const label = models == null
    ? catalogModel ? t('chats.catalog_model', { model: catalogModel }) : t('app.model_unknown')
    : models.length === 0 ? t('chats.no_model_records')
      : models.length === 1 ? models[0] ?? t('app.model_unknown')
        : `${t('chats.actual_model_count', { count: known.length })}${models.includes(null) ? t('chats.includes_unknown_model') : ''}`;
  return <span title={models?.map(model => model ?? t('app.model_unknown')).join(' · ')}>{label}</span>;
}
