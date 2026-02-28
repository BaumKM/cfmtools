use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimpleCardinalityInterval {
    lower: usize,
    upper: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum IntervalError {
    InvalidBounds { lower: usize, upper: usize },
}

impl fmt::Display for IntervalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBounds { lower, upper } => {
                write!(f, "invalid interval: lower ({lower}) > upper ({upper})")
            }
        }
    }
}
impl std::error::Error for IntervalError {}

impl SimpleCardinalityInterval {
    pub fn try_new(lower: usize, upper: Option<usize>) -> Result<Self, IntervalError> {
        if let Some(u) = upper
            && lower > u
        {
            return Err(IntervalError::InvalidBounds { lower, upper: u });
        }
        Ok(Self { lower, upper })
    }

    #[must_use]
    pub fn contains(&self, value: usize) -> bool {
        if value < self.lower {
            return false;
        }
        match self.upper {
            Some(u) => value <= u,
            None => true,
        }
    }

    #[must_use]
    pub fn lower(&self) -> usize {
        self.lower
    }

    #[must_use]
    pub fn upper(&self) -> Option<usize> {
        self.upper
    }

    #[must_use]
    pub fn size(&self) -> Option<usize> {
        let upper = self.upper?;
        Some(upper - self.lower + 1)
    }
}

impl fmt::Display for SimpleCardinalityInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.upper() {
            Some(u) => write!(f, "[{},{}]", self.lower(), u),
            None => write!(f, "[{},∞)", self.lower()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardinalityInterval {
    intervals: Vec<SimpleCardinalityInterval>,
}

impl CardinalityInterval {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            intervals: vec![
                SimpleCardinalityInterval::try_new(0, Some(0)).expect("[0,0] is always valid"),
            ],
        }
    }

    #[must_use]
    pub fn one() -> Self {
        Self {
            intervals: vec![
                SimpleCardinalityInterval::try_new(1, Some(1)).expect("[1,1] is always valid"),
            ],
        }
    }

    /// Create a normalized cardinality interval.
    ///
    /// - Empty input defaults to [0, 0].
    /// - Intervals are sorted and merged.
    #[must_use]
    pub fn new(mut intervals: Vec<SimpleCardinalityInterval>) -> Self {
        if intervals.is_empty() {
            return Self::empty();
        }

        intervals.sort_by_key(SimpleCardinalityInterval::lower);

        // merge overlapping / adjacent
        let mut merged = Vec::new();

        for iv in intervals {
            match merged.last_mut() {
                None => merged.push(iv),
                Some(last) => {
                    let can_merge = match last.upper {
                        None => true,
                        Some(last_upper) => iv.lower <= last_upper + 1, // Overlapping or adjacent
                    };

                    if can_merge {
                        last.upper = match (last.upper, iv.upper) {
                            (None, _) | (_, None) => None,
                            (Some(a), Some(b)) => Some(a.max(b)),
                        };
                    } else {
                        merged.push(iv);
                    }
                }
            }
        }

        Self { intervals: merged }
    }

    #[must_use]
    pub fn contains(&self, value: usize) -> bool {
        self.intervals.iter().any(|iv| iv.contains(value))
    }

    #[must_use]
    pub fn is_bounded(&self) -> bool {
        self.intervals.last().and_then(|i| i.upper).is_some()
    }

    #[must_use]
    pub fn max(&self) -> Option<usize> {
        self.intervals.last().and_then(|i| i.upper)
    }

    #[must_use]
    pub fn size(&self) -> Option<usize> {
        let mut total = 0usize;

        for interval in &self.intervals {
            let s = interval.size()?;
            total += s;
        }

        Some(total)
    }

    #[must_use]
    pub fn intervals(&self) -> &[SimpleCardinalityInterval] {
        &self.intervals
    }
}

impl fmt::Display for CardinalityInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for iv in &self.intervals {
            write!(f, "{iv}")?;
        }
        Ok(())
    }
}

pub struct SimpleIntervalIter {
    current: usize,
    upper: Option<usize>,
    done: bool,
}

impl Iterator for SimpleIntervalIter {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let value = self.current;

        match self.upper {
            Some(upper) => {
                if value > upper {
                    return None;
                }

                if value == upper {
                    self.done = true;
                } else {
                    self.current += 1;
                }
            }
            None => {
                // Unbounded interval: infinite iterator
                self.current += 1;
            }
        }

        Some(value)
    }
}

impl IntoIterator for &SimpleCardinalityInterval {
    type Item = usize;
    type IntoIter = SimpleIntervalIter;

    fn into_iter(self) -> Self::IntoIter {
        SimpleIntervalIter {
            current: self.lower(),
            upper: self.upper(),
            done: false,
        }
    }
}

/// Iterator over all values contained in a `CardinalityInterval`.
///
/// Values are yielded in strictly increasing order.
pub struct CardinalityIter<'a> {
    intervals: &'a [SimpleCardinalityInterval],
    index: usize,
    current: Option<SimpleIntervalIter>,
}

impl Iterator for CardinalityIter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // If we have an active sub-iterator, try it first
            if let Some(iter) = &mut self.current
                && let Some(v) = iter.next()
            {
                return Some(v);
            }

            // Move to the next interval
            let iv = self.intervals.get(self.index)?;
            self.index += 1;

            self.current = Some(iv.into_iter());
        }
    }
}

impl<'a> IntoIterator for &'a CardinalityInterval {
    type Item = usize;
    type IntoIter = CardinalityIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        CardinalityIter {
            intervals: &self.intervals,
            index: 0,
            current: None,
        }
    }
}
