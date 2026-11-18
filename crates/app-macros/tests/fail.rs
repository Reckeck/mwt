//! Signature errors the macro reports itself, with their messages pinned.

#[test]
fn refused_shapes_report_clear_errors() {
    trybuild::TestCases::new().compile_fail("tests/fail/*.rs");
}
