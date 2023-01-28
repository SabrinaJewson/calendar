#[derive(Debug)]
pub struct Log {
    pub start_date: Date,
    pub days: Vec<Day>,
}

#[derive(Debug, Clone, Default)]
pub struct Day {
    pub state: State,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum State {
    #[default]
    Unknown,
    Well,
    Sick,
    Vaccine,
}

impl Log {
    pub fn blank(start_date: Date, days: usize) -> Self {
        Self {
            start_date,
            days: vec![Day::default(); days],
        }
    }

    pub fn days(&self) -> Days<'_> {
        Days {
            next_date: self.start_date,
            days: self.days.iter(),
        }
    }
}

pub struct Days<'log> {
    next_date: Date,
    days: slice::Iter<'log, Day>,
}

impl<'log> Iterator for Days<'log> {
    type Item = (Date, &'log Day);
    fn next(&mut self) -> Option<Self::Item> {
        let date = self.next_date;
        let day = self.days.next()?;
        self.next_date = self.next_date.next_day().unwrap();
        Some((date, day))
    }
}

impl Display for Log {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for (date, day) in self.days() {
            writeln!(f, "{date}:{day}")?;
        }
        Ok(())
    }
}

impl Display for Day {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.state {
            State::Unknown => f.write_str(" ?"),
            State::Well => Ok(()),
            State::Sick => f.write_str(" S"),
            State::Vaccine => f.write_str(" V"),
        }
    }
}

impl FromStr for Log {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut start_prev_date: Option<(Date, Date)> = None;
        let mut days = Vec::new();

        for (line_num, line) in s.lines().enumerate() {
            let res = (|| {
                let (date, day) = line.split_once(':').ok_or(ParseErrorInner::NoColon)?;

                let date =
                    Date::parse(date, &Iso8601::DEFAULT).map_err(ParseErrorInner::InvalidDate)?;

                if let Some((_start_date, prev_date)) = &mut start_prev_date {
                    let expected = prev_date.next_day().unwrap();
                    if expected != date {
                        return Err(ParseErrorInner::UnexpectedDate {
                            expected,
                            found: date,
                        });
                    }
                    *prev_date = expected;
                } else {
                    start_prev_date = Some((date, date));
                }

                let state = match day.trim() {
                    "?" => State::Unknown,
                    "" => State::Well,
                    "S" => State::Sick,
                    "V" => State::Vaccine,
                    _ => return Err(ParseErrorInner::UnknownState),
                };

                Ok(Day { state })
            })();
            days.push(res.map_err(|inner| ParseError {
                inner,
                line_num: line_num + 1,
            })?);
        }

        let (start_date, _) = start_prev_date.ok_or({
            ParseError {
                inner: ParseErrorInner::NoEntries,
                line_num: 0,
            }
        })?;

        Ok(Self { start_date, days })
    }
}

#[derive(Debug)]
pub struct ParseError {
    inner: ParseErrorInner,
    line_num: usize,
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "error parsing log file on line {}", self.line_num)
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn 'static + Error)> {
        Some(&self.inner)
    }
}

#[derive(Debug)]
enum ParseErrorInner {
    NoEntries,
    NoColon,
    InvalidDate(time::error::Parse),
    UnexpectedDate { expected: Date, found: Date },
    UnknownState,
}

impl Display for ParseErrorInner {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoEntries => f.write_str("no entries found"),
            Self::NoColon => f.write_str("line does not contain colon"),
            Self::InvalidDate(_) => f.write_str("failed to parse date"),
            Self::UnexpectedDate { expected, found } => {
                write!(f, "unexpected date {found}; expected {expected}")
            }
            Self::UnknownState => f.write_str("unknown state; expected `S` or nothing"),
        }
    }
}

impl Error for ParseErrorInner {
    fn source(&self) -> Option<&(dyn 'static + Error)> {
        match self {
            Self::InvalidDate(e) => Some(e),
            _ => None,
        }
    }
}

use std::error::Error;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::slice;
use std::str::FromStr;
use time::format_description::well_known::Iso8601;
use time::Date;
