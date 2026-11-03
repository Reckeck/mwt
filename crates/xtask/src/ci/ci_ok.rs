//! `xtask ci-ok`: the required status check. `RESULTS` is the space-separated results of the
//! jobs it needs; only `success` and `skipped` pass.

use anyhow::{Context, Result, bail};

/// The first result that is neither `success` nor `skipped`.
pub fn first_failure(results: &str) -> Option<&str> {
    results
        .split_whitespace()
        .find(|result| !matches!(*result, "success" | "skipped"))
}

pub fn run() -> Result<()> {
    let results = std::env::var("RESULTS").context("RESULTS is not set")?;
    println!("results: {results}");
    match first_failure(&results) {
        Some(result) => bail!("a lane ended with `{result}`"),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_and_skipped_pass() {
        assert_eq!(first_failure("success skipped success"), None);
    }

    #[test]
    fn empty_passes() {
        assert_eq!(first_failure(""), None);
    }

    #[test]
    fn failure_cancelled_and_unknown_fail() {
        assert_eq!(first_failure("success failure skipped"), Some("failure"));
        assert_eq!(first_failure("cancelled"), Some("cancelled"));
        assert_eq!(first_failure("success weird"), Some("weird"));
    }
}
