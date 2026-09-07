//! Observation segmentation, not proof of an issued quota grant or reset cause.
//! Call independently for each account/pool/window stream, ordered by time.
use std::ops::Range;

#[derive(Clone, Copy, Debug)]
pub(crate) struct WindowSample {
    pub at: i64,
    pub reset: Option<i64>,
    pub seconds: Option<u64>,
    pub used: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Boundary {
    FirstObservation,
    DeadlineChange,
    WindowChange,
    ObservedDecrease,
    ConflictingTimestamp,
}

impl Boundary {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::FirstObservation => "first_observation",
            Self::DeadlineChange => "deadline_change",
            Self::WindowChange => "window_change",
            Self::ObservedDecrease => "observed_decrease",
            Self::ConflictingTimestamp => "conflicting_timestamp",
        }
    }
}

#[derive(Debug)]
pub(crate) struct ObservationSegment {
    pub samples: Range<usize>,
    pub boundary: Boundary,
    /// Inclusive lower bound of an uncertain boundary; NOT an exact reset time.
    pub boundary_after: Option<i64>,
}

pub(crate) fn segments(samples: &[WindowSample]) -> Vec<ObservationSegment> {
    let mut result: Vec<ObservationSegment> = Vec::new();
    for (index, current) in samples.iter().enumerate() {
        let boundary = index.checked_sub(1).and_then(|previous| {
            let previous = samples[previous];
            let changed = current.reset != previous.reset
                || current.seconds != previous.seconds
                || current.used != previous.used;
            if current.at <= previous.at && changed {
                Some(Boundary::ConflictingTimestamp)
            } else if current.seconds != previous.seconds {
                Some(Boundary::WindowChange)
            } else if current.reset != previous.reset {
                Some(Boundary::DeadlineChange)
            } else if previous
                .used
                .zip(current.used)
                .is_some_and(|(before, after)| after < before)
            {
                // Even a small drop is preserved. A decrease does not identify
                // a reset, its cause, the number of grants, or a token capacity.
                Some(Boundary::ObservedDecrease)
            } else {
                None
            }
        });
        if index == 0 || boundary.is_some() {
            result.push(ObservationSegment {
                samples: index..index + 1,
                boundary: boundary.unwrap_or(Boundary::FirstObservation),
                boundary_after: index.checked_sub(1).map(|previous| samples[previous].at),
            });
        } else if let Some(segment) = result.last_mut() {
            segment.samples.end = index + 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(at: i64, used: f64, reset: i64) -> WindowSample {
        WindowSample {
            at,
            used: Some(used),
            reset: Some(reset),
            seconds: Some(300),
        }
    }

    #[test]
    fn retains_each_deadline_and_repeated_deadline_run() {
        let result = segments(&[
            sample(1, 20., 100),
            sample(2, 30., 100),
            sample(3, 1., 200),
            sample(4, 2., 100),
        ]);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].samples, 0..2);
        assert_eq!(result[1].boundary, Boundary::DeadlineChange);
        assert_eq!(result[2].boundary_after, Some(3));
    }

    #[test]
    fn same_deadline_decrease_is_observed_not_a_verified_reset() {
        let result = segments(&[
            sample(1, 20., 100),
            sample(2, 19., 100),
            sample(3, 21., 100),
        ]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[1].samples, 1..3);
        assert_eq!(result[1].boundary, Boundary::ObservedDecrease);
        assert_eq!(result[1].boundary_after, Some(1));
    }

    #[test]
    fn duplicates_do_not_create_segments_and_conflicts_do() {
        assert_eq!(
            segments(&[sample(1, 20., 100), sample(1, 20., 100)]).len(),
            1
        );
        assert_eq!(
            segments(&[sample(1, 20., 100), sample(1, 10., 100)])[1].boundary,
            Boundary::ConflictingTimestamp
        );
        assert!(segments(&[]).is_empty());
    }

    #[test]
    fn window_and_missing_deadline_changes_are_not_silently_joined() {
        let a = sample(1, 20., 100);
        let b = WindowSample {
            at: 2,
            seconds: None,
            ..a
        };
        let c = WindowSample {
            at: 3,
            reset: None,
            ..b
        };
        let result = segments(&[a, b, c]);
        assert_eq!(result[1].boundary, Boundary::WindowChange);
        assert_eq!(result[2].boundary, Boundary::DeadlineChange);
    }
}
