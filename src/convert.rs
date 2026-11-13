//! Conversions: `#[tygr(try_from = S)]` and friends parse `S`, then convert
//! it to the field's type, or to the type they're on.

use crate::State;
use crate::grammar::{Mode, Parse};
use crate::wrappers::text;
use std::convert::Infallible;
use std::fmt::Display;
use std::ops::Range;
use std::str::FromStr;

/// `try_from = S`: parse `S`, then convert it, even when checking, as the
/// conversion can fail. A failure rejects the text, which must `requirement`,
/// saying why it doesn't.
#[inline(always)]
pub fn try_from<S: Parse + TryInto<T>, T, M: Mode>(
    input: &str,
    pos: usize,
    mut state: State<M>,
    requirement: &'static str,
) -> Option<(M::Output<T>, usize)>
where
    S::Error: Display,
{
    let (source, end) = S::build_at(input, pos, state.reborrow())?;
    let value = converted(source.try_into(), pos..end, state, requirement)?;
    Some((M::build(|_| value), end))
}

/// `try_from_infallible = S`: parse `S`, then convert it.
#[inline(always)]
pub fn try_from_infallible<S: Parse + TryInto<T, Error = Infallible>, T, M: Mode>(
    input: &str,
    pos: usize,
    state: State<M>,
) -> Option<(M::Output<T>, usize)> {
    let (source, end) = S::parse_at::<M>(input, pos, state)?;
    Some((
        M::map(source, |source| {
            let Ok(value) = source.try_into();
            value
        }),
        end,
    ))
}

/// `from_str = S`: check `S`, then convert its text, even when checking, as
/// the conversion can fail. A failure rejects the text, which must
/// `requirement`, saying why it doesn't.
#[inline(always)]
pub fn from_str<S: Parse, T: FromStr, M: Mode>(
    input: &str,
    pos: usize,
    mut state: State<M>,
    requirement: &'static str,
) -> Option<(M::Output<T>, usize)>
where
    T::Err: Display,
{
    let end = S::check_at(input, pos, state.reborrow())?;
    let value = converted(
        T::from_str(text(input, pos, end)),
        pos..end,
        state,
        requirement,
    )?;
    Some((M::build(|_| value), end))
}

/// `from_str_infallible = S`: check `S`, then convert its text.
#[inline(always)]
pub fn from_str_infallible<S: Parse, T: FromStr<Err = Infallible>, M: Mode>(
    input: &str,
    pos: usize,
    state: State<M>,
) -> Option<(M::Output<T>, usize)> {
    let end = S::check_at(input, pos, state)?;
    Some((
        M::build(|_| {
            let Ok(value) = T::from_str(text(input, pos, end));
            value
        }),
        end,
    ))
}

/// The converted value, or `None` once the text at `range` is recorded as
/// rejected: it must `requirement`, and the error says why it doesn't.
#[inline(always)]
fn converted<M: Mode, T, E: Display>(
    result: Result<T, E>,
    range: Range<usize>,
    mut state: State<M>,
    requirement: &'static str,
) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            let why = if M::RECORD {
                format!("{requirement}: {error}")
            } else {
                String::new()
            };
            state.reject(None, range, why);
            None
        }
    }
}
