//! The workflow area's pure functions over the vault document and the read
//! context (spec core-strangler, F2): records and their files, runs and the
//! journal, the marks a workflow keeps, variables and the lookups the nodes
//! make, and the tests a user keeps with a workflow. The scenarios that write
//! are in `usecases::workflow`.

pub mod marks;
pub mod records;
pub mod runs;
pub mod tests_def;
pub mod vars;
