use super::*;
use std::cell::RefCell;

const MAX_ENTRIES: usize = 64;
const MAX_ESTIMATED_BYTES: usize = 8 * 1024 * 1024;

#[derive(PartialEq, Eq)]
pub(super) struct SeriesKey {
    grain: TimeGrain,
    dimension: Option<AggregateDimension>,
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
    account: Option<String>,
    project: Option<String>,
    model: Option<String>,
    quality: Option<DataQuality>,
    timezone: String,
    local_changes: u64,
}

impl SeriesKey {
    pub(super) fn new(
        grain: TimeGrain,
        dimension: Option<AggregateDimension>,
        filter: &AggregateFilter,
        timezone: &str,
        local_changes: u64,
    ) -> Self {
        Self {
            grain,
            dimension,
            start: filter.start_inclusive,
            end: filter.end_exclusive,
            account: filter.account_fingerprint.clone(),
            project: filter.project_id.clone(),
            model: filter.model.clone(),
            quality: filter.quality,
            timezone: timezone.into(),
            local_changes,
        }
    }
    fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.timezone.len()
            + self.account.as_ref().map_or(0, String::len)
            + self.project.as_ref().map_or(0, String::len)
            + self.model.as_ref().map_or(0, String::len)
    }
}

#[derive(Default)]
pub(super) struct SnapshotMemo {
    entries: Vec<(SeriesKey, Vec<UsageSeriesBucket>)>,
    estimated_bytes: usize,
    #[cfg(test)]
    pub(super) hits: usize,
}

impl SnapshotMemo {
    pub(super) fn get(&mut self, key: &SeriesKey) -> Option<Vec<UsageSeriesBucket>> {
        let value = self
            .entries
            .iter()
            .find(|(stored, _)| stored == key)
            .map(|(_, value)| value.clone());
        #[cfg(test)]
        if value.is_some() {
            self.hits += 1;
        }
        value
    }
    pub(super) fn insert(&mut self, key: SeriesKey, value: &[UsageSeriesBucket]) {
        if self.entries.len() >= MAX_ENTRIES
            || self.entries.iter().any(|(stored, _)| stored == &key)
        {
            return;
        }
        let bytes = value.iter().fold(
            key.estimated_bytes() + std::mem::size_of::<Vec<UsageSeriesBucket>>(),
            |sum, row| {
                sum.saturating_add(std::mem::size_of::<UsageSeriesBucket>())
                    .saturating_add(row.time_key.len())
                    .saturating_add(row.dimension_key.as_ref().map_or(0, String::len))
            },
        );
        if self.estimated_bytes.saturating_add(bytes) > MAX_ESTIMATED_BYTES {
            return;
        }
        self.estimated_bytes += bytes;
        self.entries.push((key, value.to_vec()));
    }
}

pub(super) struct MemoScope<'a>(&'a RefCell<Option<SnapshotMemo>>);
impl<'a> MemoScope<'a> {
    pub(super) fn begin(cell: &'a RefCell<Option<SnapshotMemo>>) -> Self {
        cell.replace(Some(SnapshotMemo::default()));
        Self(cell)
    }
}
impl Drop for MemoScope<'_> {
    fn drop(&mut self) {
        self.0.replace(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn memo_rejects_oversized_values_and_bounds_empty_entries() {
        let mut memo = SnapshotMemo::default();
        let key = || SeriesKey::new(TimeGrain::Hour, None, &AggregateFilter::default(), "UTC", 0);
        let row = UsageSeriesBucket {
            time_key: "x".repeat(MAX_ESTIMATED_BYTES),
            dimension_key: None,
            event_count: 0,
            usage: TokenUsage::default(),
        };
        memo.insert(key(), &[row]);
        assert!(memo.get(&key()).is_none());
        for index in 0..MAX_ENTRIES + 1 {
            memo.insert(
                SeriesKey::new(
                    TimeGrain::Hour,
                    None,
                    &AggregateFilter::default(),
                    "UTC",
                    index as u64,
                ),
                &[],
            );
        }
        assert_eq!(memo.entries.len(), MAX_ENTRIES);
        assert!(memo.estimated_bytes <= MAX_ESTIMATED_BYTES);
    }
}
