//! Field conversions: `#[tygr(try_from = S)]` and friends parse `S`, then
//! convert it to the field's type.

use crate::State;
use crate::grammar::{Build, Check, Mode, Parse};
use crate::wrappers::text;
use std::convert::Infallible;
use std::ops::Range;
use std::str::FromStr;

/// `try_from = S`: parse `S`, then convert it, even when checking, as the
/// conversion can fail. A failure rejects the text, which must `requirement`.
#[inline(always)]
pub fn try_from<S: Parse + TryInto<T>, T, M: Mode>(
    input: &[u8],
    pos: usize,
    mut state: State,
    requirement: &'static str,
) -> Option<(M::Output<T>, usize)> {
    let (source, end) = S::parse_at::<Build>(input, pos, state.reborrow())?;
    let value = converted(source.try_into(), pos..end, state, requirement)?;
    Some((M::build(|_| value), end))
}

/// `try_from_infallible = S`: parse `S`, then convert it.
#[inline(always)]
pub fn try_from_infallible<S: Parse + TryInto<T, Error = Infallible>, T, M: Mode>(
    input: &[u8],
    pos: usize,
    state: State,
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
/// `requirement`.
#[inline(always)]
pub fn from_str<S: Parse, T: FromStr, M: Mode>(
    input: &[u8],
    pos: usize,
    mut state: State,
    requirement: &'static str,
) -> Option<(M::Output<T>, usize)> {
    let ((), end) = S::parse_at::<Check>(input, pos, state.reborrow())?;
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
    input: &[u8],
    pos: usize,
    state: State,
) -> Option<(M::Output<T>, usize)> {
    let ((), end) = S::parse_at::<Check>(input, pos, state)?;
    Some((
        M::build(|_| {
            let Ok(value) = T::from_str(text(input, pos, end));
            value
        }),
        end,
    ))
}

/// The converted value, or `None` once the text at `range` is recorded as
/// rejected.
#[inline(always)]
fn converted<T, E>(
    result: Result<T, E>,
    range: Range<usize>,
    mut state: State,
    requirement: &'static str,
) -> Option<T> {
    if result.is_err() {
        state.reject(None, range, requirement);
    }
    result.ok()
}
