#[derive(Debug)]
pub(crate) struct Log {
    highlights: Vec<Highlight>,
    start_date: Date,
    days: Vec<Day>,
}

impl FromStr for Log {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        toml::Deserializer::new(s)
            .deserialize_map(DeVisitor)
            .map_err(ParseError)
    }
}

#[derive(Debug)]
pub(crate) struct ParseError(toml::de::Error);

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("failed to parse log")
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

impl Log {
    pub fn start_date(&self) -> Date {
        self.start_date
    }

    pub fn days(&self) -> Days<'_> {
        Days {
            highlights: &self.highlights,
            iter: self.days.iter(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Days<'log> {
    highlights: &'log [Highlight],
    iter: slice::Iter<'log, Day>,
}

impl<'log> Iterator for Days<'log> {
    type Item = Option<&'log Highlight>;

    fn next(&mut self) -> Option<Self::Item> {
        let day = self.iter.next()?;
        Some(day.highlight().map(|i| &self.highlights[i]))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
}
impl ExactSizeIterator for Days<'_> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

struct DeVisitor;

impl<'de> de::Visitor<'de> for DeVisitor {
    type Value = Log;
    fn expecting(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("a table")
    }
    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let index: HighlightIndex = de_map_access_require_entry(&mut map, "highlights")?;
        let seed = data::DeserializeSeed {
            indices: &index.indices,
        };
        let data = de_map_access_require_entry_seed(&mut map, "data", seed)?;
        Ok(Log {
            highlights: index.highlights,
            start_date: data.start_date,
            days: data.days,
        })
    }
}

struct HighlightIndex {
    highlights: Vec<Highlight>,
    indices: ahash::HashMap<String, usize>,
}

impl<'de> Deserialize<'de> for HighlightIndex {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(HighlightsVisitor)
    }
}

struct HighlightsVisitor;
impl<'de> de::Visitor<'de> for HighlightsVisitor {
    type Value = HighlightIndex;
    fn expecting(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("a table")
    }
    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut index = HighlightIndex {
            highlights: Vec::new(),
            indices: HashMap::default(),
        };
        while let Some((key, value)) = map.next_entry()? {
            if index.indices.contains_key(&key) {
                return Err(de::Error::custom(format_args!("duplicate highlight {key}")));
            }
            index.indices.insert(key, index.highlights.len());
            index.highlights.push(value);
        }
        Ok(index)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Highlight {
    pub shape: Shape,
    pub colour: Colour,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Shape {
    Rectangle,
    Circle,
}

mod colour {
    #[derive(Debug)]
    pub(crate) struct Colour(pub [u8; 3]);

    impl<'de> Deserialize<'de> for Colour {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            deserializer.deserialize_str(DeVisitor)
        }
    }

    struct DeVisitor;

    impl<'de> de::Visitor<'de> for DeVisitor {
        type Value = Colour;
        fn expecting(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str("a colour")
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            let v = v
                .strip_prefix('#')
                .ok_or_else(|| E::custom("colour must start with #"))?;
            <&[u8; 6]>::try_from(v.as_bytes())
                .ok()
                .and_then(parse_6_hex)
                .map(Colour)
                .ok_or_else(|| de::Error::custom("colour must contain 6 hex digits"))
        }
    }

    fn parse_6_hex(v: &[u8; 6]) -> Option<[u8; 3]> {
        let mut simd = <Simd<u8, 8>>::splat(b'0');
        simd.as_mut_array()[..6].copy_from_slice(v);
        let len_09 = Simd::splat(b'9' - b'0' + 1);
        let len_af = Simd::splat(b'F' - b'A' + 1);
        let not_09 = (simd - Simd::splat(b'0')).simd_ge(len_09);
        let not_af = (simd - Simd::splat(b'A')).simd_ge(len_af);
        if (not_09 & not_af).any() {
            return None;
        }
        let values = not_09.select(simd - Simd::splat(b'A') + len_09, simd - Simd::splat(b'0'));
        let higher = simd_swizzle!(values, [0, 2, 4, 6]) << Simd::splat(4);
        let lower = simd_swizzle!(values, [1, 3, 5, 7]);
        let data = higher | lower;
        Some(data[..3].try_into().unwrap())
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn parse_hex_works() {
            assert_eq!(parse_6_hex(b"C92DA1"), Some([0xC9, 0x2D, 0xA1]));
            assert_eq!(parse_6_hex(b"4AA4B9"), Some([0x4A, 0xA4, 0xB9]));
            assert_eq!(parse_6_hex(b"4AA4B/"), None);
            assert_eq!(parse_6_hex(b":AA4B9"), None);
            assert_eq!(parse_6_hex(b"4AA@B9"), None);
            assert_eq!(parse_6_hex(b"4AG4B9"), None);
        }

        use crate::log::colour::parse_6_hex;
    }

    use serde::de;
    use serde::Deserialize;
    use serde::Deserializer;
    use std::fmt;
    use std::fmt::Formatter;
    use std::simd::simd_swizzle;
    use std::simd::Simd;
    use std::simd::SimdPartialOrd as _;
}
pub(crate) use colour::Colour;

mod data {
    #[derive(Debug)]
    pub(super) struct Data {
        pub start_date: Date,
        pub days: Vec<Day>,
    }

    pub(super) struct DeserializeSeed<'map, S: BuildHasher> {
        pub indices: &'map HashMap<String, usize, S>,
    }

    impl<'de, S: BuildHasher> de::DeserializeSeed<'de> for DeserializeSeed<'_, S> {
        type Value = Data;
        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_map(self)
        }
    }

    impl<'de, S: BuildHasher> de::Visitor<'de> for DeserializeSeed<'_, S> {
        type Value = Data;
        fn expecting(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str("a data table")
        }
        fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let start_date = map
                .next_key::<Date>()?
                .ok_or_else(|| de::Error::invalid_length(0, &"a non-empty table"))?;
            let mut current_date = start_date;
            let mut days = Vec::new();
            loop {
                days.push(map.next_value_seed(WrappedDay {
                    indices: self.indices,
                    date: current_date,
                })?);
                current_date = current_date.next_day().unwrap();
                match map.next_key_seed(Exact(current_date))? {
                    Some(()) => {}
                    None => break,
                }
            }
            Ok(Data { start_date, days })
        }
    }

    struct WrappedDay<'map, S: BuildHasher> {
        indices: &'map HashMap<String, usize, S>,
        date: Date,
    }

    impl<'de, S: BuildHasher> serde::de::DeserializeSeed<'de> for WrappedDay<'_, S> {
        type Value = Day;
        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_map(self)
        }
    }

    impl<'de, S: BuildHasher> de::Visitor<'de> for WrappedDay<'_, S> {
        type Value = Day;
        fn expecting(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str("a map")
        }
        fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            map.next_key_seed(LiteralStr(match self.date.weekday() {
                Weekday::Monday => "Mon",
                Weekday::Tuesday => "Tue",
                Weekday::Wednesday => "Wed",
                Weekday::Thursday => "Thu",
                Weekday::Friday => "Fri",
                Weekday::Saturday => "Sat",
                Weekday::Sunday => "Sun",
            }))?;
            map.next_value_seed(day::DeserializeSeed {
                indices: self.indices,
            })
        }
    }

    use super::day;
    use super::util::LiteralStr;
    use super::Day;
    use super::util::Exact;
    use serde::de;
    use serde::Deserializer;
    use std::collections::HashMap;
    use std::fmt;
    use std::fmt::Formatter;
    use std::hash::BuildHasher;
    use time::Date;
    use time::Weekday;
}

mod day {
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct Day {
        // `usize::MAX` if there is no highlight
        highlight: usize,
    }

    impl Day {
        pub(crate) fn highlight(self) -> Option<usize> {
            if self.highlight == usize::MAX {
                None
            } else {
                Some(self.highlight)
            }
        }
    }

    pub(super) struct DeserializeSeed<'map, S: BuildHasher> {
        pub indices: &'map HashMap<String, usize, S>,
    }
    impl<'de, S: BuildHasher> serde::de::DeserializeSeed<'de> for DeserializeSeed<'_, S> {
        type Value = Day;
        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_str(self)
        }
    }
    impl<'de, S: BuildHasher> de::Visitor<'de> for DeserializeSeed<'_, S> {
        type Value = Day;
        fn expecting(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str("a string")
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            if v.is_empty() {
                return Ok(Day {
                    highlight: usize::MAX,
                });
            }
            let highlight = *self
                .indices
                .get(v)
                .ok_or_else(|| E::custom(format_args!("no known highlight `{v}`")))?;
            Ok(Day { highlight })
        }
    }

    use serde::de;
    use serde::Deserializer;
    use std::collections::HashMap;
    use std::fmt;
    use std::fmt::Formatter;
    use std::hash::BuildHasher;
}
pub(crate) use day::Day;

mod util {
    pub(crate) fn de_map_access_require_entry<'de, T, A>(
        map: &mut A,
        key: &'static str,
    ) -> Result<T, A::Error>
    where
        T: Deserialize<'de>,
        A: de::MapAccess<'de>,
    {
        de_map_access_require_entry_seed(map, key, PhantomData::<T>)
    }

    pub(crate) fn de_map_access_require_entry_seed<'de, S, A>(
        map: &mut A,
        key: &'static str,
        seed: S,
    ) -> Result<S::Value, A::Error>
    where
        S: DeserializeSeed<'de>,
        A: de::MapAccess<'de>,
    {
        map.next_key_seed(LiteralStr(key))?
            .ok_or_else(|| de::Error::missing_field(key))?;
        map.next_value_seed(seed)
    }

    mod literal_str {
        pub(crate) struct LiteralStr<'s>(pub &'s str);

        impl<'de> DeserializeSeed<'de> for LiteralStr<'_> {
            type Value = ();

            fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                deserializer.deserialize_str(self)
            }
        }

        impl<'de> de::Visitor<'de> for LiteralStr<'_> {
            type Value = ();
            fn expecting(&self, f: &mut Formatter<'_>) -> fmt::Result {
                write!(f, "the string `{}`", self.0)
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                if v != self.0 {
                    return Err(de::Error::invalid_value(de::Unexpected::Str(v), &self));
                }
                Ok(())
            }
        }

        use serde::de;
        use serde::de::DeserializeSeed;
        use serde::de::Deserializer;
        use std::fmt;
        use std::fmt::Formatter;
    }
    pub(crate) use literal_str::LiteralStr;

    mod exact {
        pub(crate) struct Exact<T>(pub T);

        impl<'de, T: Deserialize<'de> + PartialEq + Display> DeserializeSeed<'de> for Exact<T> {
            type Value = ();

            fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                let Self(expected) = self;
                let value = T::deserialize(deserializer)?;
                if value != expected {
                    return Err(de::Error::custom(format_args!(
                        "invalid value: {value}, expected {expected}"
                    )));
                }
                Ok(())
            }
        }

        use serde::de;
        use serde::de::DeserializeSeed;
        use serde::Deserialize;
        use serde::Deserializer;
        use std::fmt::Display;
    }
    pub(crate) use exact::Exact;

    use serde::de;
    use serde::de::DeserializeSeed;
    use serde::Deserialize;
    use std::marker::PhantomData;
}
use util::de_map_access_require_entry;

use self::util::de_map_access_require_entry_seed;
use serde::de;
use serde::Deserialize;
use serde::Deserializer;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::slice;
use std::str::FromStr;
use time::Date;
