//! Conversions: `#[tygr(try_from = S)]` and friends parse `S`, then convert
//! it to the field's type, or to the type they're on.

use crate::grammar::{Parse, Success, Unit, Value};
use crate::wrappers::text;
use crate::{Expectation, Level, State};
use std::convert::Infallible;
use std::fmt::Display;
use std::ops::Range;
use std::str::FromStr;

/// `try_from = S`: parse `S`, then convert it, even when checking, as the
/// conversion can fail. A failure records the text as not convertible
/// `into` the type, with the error.
#[inline(always)]
pub fn try_from<Src: Parse + TryInto<T>, T, S: Success, L: Level>(
    input: &str,
    pos: usize,
    mut state: State<L>,
    into: &'static str,
) -> Option<(S::Output<T>, usize)>
where
    Src::Error: Display,
{
    let (source, end) = Src::parse_at::<Value, L>(input, pos, state.reborrow())?;
    let value = converted(source.try_into(), pos..end, state, into)?;
    Some((S::output(|| value), end))
}

/// `try_from_infallible = S`: parse `S`, then convert it.
#[inline(always)]
pub fn try_from_infallible<Src: Parse + TryInto<T, Error = Infallible>, T, S: Success, L: Level>(
    input: &str,
    pos: usize,
    state: State<L>,
) -> Option<(S::Output<T>, usize)> {
    let (source, end) = Src::parse_at::<S, L>(input, pos, state)?;
    Some((
        S::and_then(source, |source| {
            S::output(|| {
                let Ok(value) = source.try_into();
                value
            })
        }),
        end,
    ))
}

/// `from_str = S`: check `S`, then convert its text, even when checking, as
/// the conversion can fail. A failure records the text as not convertible
/// `into` the type, with the error.
#[inline(always)]
pub fn from_str<Src: Parse, T: FromStr, S: Success, L: Level>(
    input: &str,
    pos: usize,
    mut state: State<L>,
    into: &'static str,
) -> Option<(S::Output<T>, usize)>
where
    T::Err: Display,
{
    let ((), end) = Src::parse_at::<Unit, L>(input, pos, state.reborrow())?;
    let value = converted(T::from_str(text(input, pos, end)), pos..end, state, into)?;
    Some((S::output(|| value), end))
}

/// `from_str_infallible = S`: check `S`, then convert its text.
#[inline(always)]
pub fn from_str_infallible<Src: Parse, T: FromStr<Err = Infallible>, S: Success, L: Level>(
    input: &str,
    pos: usize,
    state: State<L>,
) -> Option<(S::Output<T>, usize)> {
    let ((), end) = Src::parse_at::<Unit, L>(input, pos, state)?;
    Some((
        S::output(|| {
            let Ok(value) = T::from_str(text(input, pos, end));
            value
        }),
        end,
    ))
}

/// The converted value, or `None` once the text at `range` is recorded as
/// not convertible `into` the type.
#[inline(always)]
fn converted<L: Level, T, E: Display>(
    result: Result<T, E>,
    range: Range<usize>,
    mut state: State<L>,
    into: &'static str,
) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            state.expect(range.end, || Expectation::Convertible {
                into,
                range,
                error: error.to_string().into(),
            });
            None
        }
    }
}
