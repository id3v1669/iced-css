//! Build-time diagnostics of the `#[iced_css::style]` macro, via trybuild.
//! Lives here (not in iced-css-macro) because the macro crate cannot depend
//! on iced-css.
//!
//! SKELETON: the macro is currently a passthrough, so the compile-fail cases
//! still compile and this suite fails (red) — as intended.

#[test]
fn macro_diagnostics() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/pass/*.rs");
    t.compile_fail("tests/ui/fail/*.rs");
}
