//! The accepted shapes compile against a stub of `crate::ipc` (no Tauri).
//! Error cases are `compile_fail` doc tests in `src/lib.rs`.

#[test]
fn accepted_shapes_compile() {
    trybuild::TestCases::new().pass("tests/pass/*.rs");
}
