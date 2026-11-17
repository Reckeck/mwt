//! The kernel of the core: the one place for how a change of the open vault
//! is committed. It grows by the strangler rule; the first step only pins the
//! write path as it is, so the later moves can be checked against it.

#[cfg(test)]
mod golden_tests;
