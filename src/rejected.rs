//! Inputs the derives reject, as `compile_fail` doc tests.

/// ```compile_fail
/// #[derive(tygr::Parse)]
/// #[tygr(unknown)]
/// struct S;
/// ```
fn unknown_option() {}

/// ```compile_fail
/// #[derive(tygr::Parse)]
/// #[tygr(unbounded(X))]
/// struct S<T>(T);
/// ```
fn unbounded_non_parameter() {}

/// ```compile_fail
/// #[derive(tygr::Parse)]
/// struct S(#[tygr(try_from = (), from_str = ())] ());
/// ```
fn two_conversions() {}

/// ```compile_fail
/// #[derive(tygr::Parse)]
/// enum E {
///     #[tygr(from_str = ())]
///     Unit,
/// }
/// ```
fn unit_variant_from_str() {}

/// ```compile_fail
/// #[derive(tygr::Grammar)]
/// enum E {
///     #[tygr(print_to = tygr::print_display)]
///     Unit,
/// }
/// ```
fn unit_variant_print_to_without_source() {}

/// ```compile_fail
/// #[derive(tygr::Parse)]
/// enum E {
///     #[tygr(try_from = ())]
///     Tuple(()),
/// }
/// ```
fn attribute_on_non_unit_variant() {}

/// ```compile_fail
/// #[derive(tygr::Grammar)]
/// struct S(#[tygr(from_str = ())] u32);
/// ```
fn unprintable_converted_field() {}

/// ```compile_fail
/// #[derive(tygr::Parse)]
/// union U {
///     a: u8,
/// }
/// ```
fn union() {}

/// ```compile_fail,E0277
/// #[derive(tygr::Grammar)]
/// struct Port(u16);
/// ```
fn field_that_is_no_grammar() {}
