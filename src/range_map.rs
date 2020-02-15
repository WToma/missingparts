use std::cmp::Ordering;

/// A map-like structure, whose speciality is that it can efficiently hold values for ranges of keys.
///
/// Deletions are not supported. If an entry is removed, the entire map should be constructed again.
///
/// # Examples
/// ```
/// # use missingparts::range_map::*;
/// # use std::collections::Set;
/// // keeps track of events in our convention center
/// let events = RangeMap::new();
///
/// // note: a real use case should use a proper date type, instead of
/// // strings, but strings work as well
/// events.insert("2020-02-09", "2020-02-23", "Rust Conference"); // event is from 9th to 22th: end is exclusive
/// events.insert("2020-02-20", "2020-03-02", "Spring Roll Festival"); // event is from 20th to 1st: end is exclusive
///
/// for (i, (date_range, events_in_date_range)) in events.reverse_iterator().enumerate() {
///     match i {
///         0 => {
///             assert_eq!(date_range.start, "2020-02-23");
///             assert_eq!(date_range.end, "2020-03-02");
///             assert_eq!(events_in_date_range, vec!["Spring Roll Festival"]);
///         }
///         1 => {
///             assert_eq!(date_range.start, "2020-02-20");
///             assert_eq!(date_range.end, "2020-02-23");
///             assert_eq!(events_in_date_range, vec!["Rust Conference", "Spring Roll Festival"]);
///         }
///         2 => {
///             assert_eq!(date_range.start, "2020-02-09");
///             assert_eq!(date_range.end, "2020-02-20");
///             assert_eq!(events_in_date_range, vec!["Rust Conference"]);
///         }
///         _ => panic!("Too many ranges returned")
///     }
/// }
/// ```
pub struct RangeMap<K: Ord + Copy, V: Clone + Copy> {
    non_overlapping_ranges: Vec<(NonOverlappingRange<K>, Vec<V>)>,
}

impl<K: Ord + Copy, V: Clone + Copy> RangeMap<K, V> {
    /// Creates a new instance of the range map. Refer to the struct-level example to see
    /// how to use it.
    pub fn new() -> RangeMap<K, V> {
        RangeMap {
            non_overlapping_ranges: Vec::new(),
        }
    }

    /// Inserts the value V for the range `[new_range_start, new_range_end)` (start inclusive, end exclusive).
    ///
    /// Note: if `new_range_end >= new_range_start`, this is a no-op.
    pub fn insert(&mut self, new_range_start: K, new_range_end: K, value: V) {
        if new_range_end <= new_range_start {
            return;
        }

        // TODO improvements:
        // 1. after the initial binary search we do not need to repeat the searches, the Ok(i) / Err(i) can be deduced
        //    in constant time
        // 2. maybe replace recursion with a loop?
        match self
            .non_overlapping_ranges
            .binary_search_by_key(&new_range_start, |r| r.0.start)
        {
            Ok(i) => {
                // exact match: the new range starts exactly where the old range starts
                let existing_range = &self.non_overlapping_ranges[i].0;
                if existing_range.end > new_range_end {
                    // the new range ends inside the existing range: split the existing range into two,
                    // and add the new value to the lower half
                    let old_end = existing_range.end;
                    let old_range_values = self.non_overlapping_ranges[i].1.clone();
                    self.non_overlapping_ranges[i].0.end = new_range_end;
                    self.non_overlapping_ranges[i].1.push(value);
                    self.non_overlapping_ranges.insert(
                        i + 1,
                        (
                            NonOverlappingRange {
                                start: new_range_end,
                                end: old_end,
                            },
                            old_range_values,
                        ),
                    );
                } else {
                    // the new range ends outside the existing range: add the new value to the existing range, "consume"
                    // the new range up to the end of the existing range, then recurse
                    let existing_range_end = existing_range.end;
                    self.non_overlapping_ranges[i].1.push(value);
                    self.insert(existing_range_end, new_range_end, value);
                }
            }
            Err(i) => {
                // the new range starts between some other ranges (or there are no other ranges)
                if i < self.non_overlapping_ranges.len()
                    && self.non_overlapping_ranges[i].0.start < new_range_end
                {
                    // there is a next range, and the next range starts before the new range ends. add the new value to
                    // a new range, "consume" the new range up to the beginning of the next range, then recurse
                    let next_range_start = self.non_overlapping_ranges[i].0.start;
                    self.non_overlapping_ranges.insert(
                        i,
                        (
                            NonOverlappingRange {
                                start: new_range_start,
                                end: next_range_start,
                            },
                            vec![value],
                        ),
                    );
                    self.insert(next_range_start, new_range_end, value);
                } else {
                    // there is no next range, or the next range starts further out than the current range starts. just
                    // add the new value into a new range
                    self.non_overlapping_ranges.insert(
                        i,
                        (
                            NonOverlappingRange {
                                start: new_range_start,
                                end: new_range_end,
                            },
                            vec![value],
                        ),
                    );
                }
            }
        }
    }
}

/// A range that does not overlap with any other ranges in the range map. Ranges are comparable to work with `RangeMap`,
/// their comparison is by the start of the range. Since we're assuming non-overlapping, this provides total ordering.
///
/// The `start` is inclusive and the `end` is exclusive, so if `start == end` that means that the range contains exactly
/// `start`.
///
/// Note: `end < start` is an undefined situation. The `RangeMap` will never return ranges like that. If you modify
/// the range to violate this invariant, you're on your own.
pub struct NonOverlappingRange<K: Ord + Copy> {
    /// The start of the range, inclusive.
    pub start: K,

    /// The end of the range, exclusive.
    pub end: K,
}

// ------------------
// impls for ordering
// ------------------
impl<K: Ord + Copy> Ord for NonOverlappingRange<K> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.start.cmp(&other.start)
    }
}

impl<K: Ord + Copy> PartialOrd for NonOverlappingRange<K> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K: Ord + Copy> PartialEq for NonOverlappingRange<K> {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start
    }
}

impl<K: Ord + Copy> Eq for NonOverlappingRange<K> {}
// ------------------
// END impls for ordering
// ------------------

#[cfg(test)]
mod tests {

    use crate::range_map::*;

    #[test]
    fn test_single_insert() {
        let mut r: RangeMap<u32, char> = RangeMap::new();
        r.insert(3, 5, 'a');

        assert_eq!(r.non_overlapping_ranges.len(), 1);
        assert_eq!(r.non_overlapping_ranges[0].0.start, 3);
        assert_eq!(r.non_overlapping_ranges[0].0.end, 5);
        assert_eq!(r.non_overlapping_ranges[0].1, vec!['a']);
    }

    #[test]
    fn test_exact_overlap() {
        let mut r: RangeMap<u32, char> = RangeMap::new();
        r.insert(3, 5, 'a');
        r.insert(3, 5, 'b'); // same as the first range

        assert_eq!(r.non_overlapping_ranges.len(), 1); // there is a single range
        assert_eq!(r.non_overlapping_ranges[0].0.start, 3);
        assert_eq!(r.non_overlapping_ranges[0].0.end, 5);
        assert_eq!(r.non_overlapping_ranges[0].1, vec!['a', 'b']); // containing both values
    }

    #[test]
    fn test_new_range_extends_forward() {
        let mut r: RangeMap<u32, char> = RangeMap::new();
        r.insert(3, 5, 'a');
        r.insert(3, 7, 'b'); // the new range starts at the same point as the old one, but extends forward

        assert_eq!(r.non_overlapping_ranges.len(), 2);

        // 1st range contains both
        assert_eq!(r.non_overlapping_ranges[0].0.start, 3);
        assert_eq!(r.non_overlapping_ranges[0].0.end, 5);
        assert_eq!(r.non_overlapping_ranges[0].1, vec!['a', 'b']);

        // 2nd range only contains the new one
        assert_eq!(r.non_overlapping_ranges[1].0.start, 5);
        assert_eq!(r.non_overlapping_ranges[1].0.end, 7);
        assert_eq!(r.non_overlapping_ranges[1].1, vec!['b']);
    }

    #[test]
    fn test_new_range_extends_backwards() {
        let mut r: RangeMap<u32, char> = RangeMap::new();
        r.insert(3, 5, 'a');
        r.insert(1, 5, 'b'); // the new range ends at the same point as the old one, but extends backwards

        assert_eq!(r.non_overlapping_ranges.len(), 2);

        // 1st range contains just the new one
        assert_eq!(r.non_overlapping_ranges[0].0.start, 1);
        assert_eq!(r.non_overlapping_ranges[0].0.end, 3);
        assert_eq!(r.non_overlapping_ranges[0].1, vec!['b']);

        // 2nd range contains both
        assert_eq!(r.non_overlapping_ranges[1].0.start, 3);
        assert_eq!(r.non_overlapping_ranges[1].0.end, 5);
        assert_eq!(r.non_overlapping_ranges[1].1, vec!['a', 'b']);
    }

    #[test]
    fn test_new_range_shortends_forward() {
        let mut r: RangeMap<u32, char> = RangeMap::new();
        r.insert(3, 7, 'a');
        r.insert(3, 5, 'b'); // the new range starts at the same point as the old one, but is shorter

        assert_eq!(r.non_overlapping_ranges.len(), 2);

        // 1st range contains both
        assert_eq!(r.non_overlapping_ranges[0].0.start, 3);
        assert_eq!(r.non_overlapping_ranges[0].0.end, 5);
        assert_eq!(r.non_overlapping_ranges[0].1, vec!['a', 'b']);

        // 2nd range only contains the old one
        assert_eq!(r.non_overlapping_ranges[1].0.start, 5);
        assert_eq!(r.non_overlapping_ranges[1].0.end, 7);
        assert_eq!(r.non_overlapping_ranges[1].1, vec!['a']);
    }

    #[test]
    fn test_new_range_shortends_backwards() {
        let mut r: RangeMap<u32, char> = RangeMap::new();
        r.insert(3, 7, 'a');
        r.insert(5, 7, 'b'); // the new range ends at the same point as the old one, but is shorter

        assert_eq!(r.non_overlapping_ranges.len(), 2);

        // 1st range contains just the old one
        assert_eq!(r.non_overlapping_ranges[0].0.start, 3);
        assert_eq!(r.non_overlapping_ranges[0].0.end, 5);
        assert_eq!(r.non_overlapping_ranges[0].1, vec!['a']);

        // 2nd range only contains both
        assert_eq!(r.non_overlapping_ranges[1].0.start, 5);
        assert_eq!(r.non_overlapping_ranges[1].0.end, 7);
        assert_eq!(r.non_overlapping_ranges[1].1, vec!['a', 'b']);
    }

    #[test]
    fn test_partial_overlap_start() {
        unimplemented!("new.start inside old");
    }

    #[test]
    fn test_partial_overlap_end() {
        unimplemented!("new.end inside old");
    }

    #[test]
    fn test_cover_no_gap() {
        unimplemented!("the new range spans 2 or more ranges, the existing ranges don't have gaps");
    }

    #[test]
    fn test_cover_gap() {
        unimplemented!(
            "the new range spans 2 or more ranges, the existing ranges have gaps between them"
        );
    }
}
