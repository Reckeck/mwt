//! `xtask release-plan`: what a release.yml run builds and whether it publishes.
//!
//! Push: development -> dev pre-release, main -> prod release. By hand (workflow_dispatch): the
//! chosen channel; it publishes only from its own branch (dev from development, prod from main).
//! Outputs: channel (dev|prod), publish, pre (a pre-release X.Y.Z-N), kind (dev|prod|none: what
//! `xtask version --channel` takes, none = a build that publishes nothing).

use anyhow::Result;

#[derive(Debug, PartialEq, Eq)]
pub struct Plan {
    pub channel: &'static str,
    pub publish: bool,
    pub pre: bool,
    pub kind: &'static str,
}

/// Inputs left unset or empty count as absent (the workflow passes empty strings on a push).
pub fn decide(
    event: &str,
    reference: &str,
    input_channel: Option<&str>,
    input_publish: Option<&str>,
) -> Result<Plan, String> {
    let (channel, publish) = match event {
        "push" => match reference {
            "development" => ("dev", true),
            "main" => ("prod", true),
            other => {
                return Err(format!(
                    "release.yml publishes from development and main only, not {other}"
                ));
            }
        },
        "workflow_dispatch" => {
            let channel = input_channel
                .filter(|text| !text.is_empty())
                .unwrap_or("dev");
            let publish = input_publish
                .filter(|text| !text.is_empty())
                .unwrap_or("false");
            let channel = match channel {
                "dev" => "dev",
                "prod" => "prod",
                other => return Err(format!("channel must be dev or prod, not {other}")),
            };
            let publish = match publish {
                "true" => true,
                "false" => false,
                other => return Err(format!("publish must be true or false, not {other}")),
            };
            let own_branch = if channel == "dev" {
                "development"
            } else {
                "main"
            };
            if publish && reference != own_branch {
                return Err(format!(
                    "publish needs the dev channel on development or the prod channel on main (this run: {channel} on {reference})"
                ));
            }
            (channel, publish)
        }
        other => return Err(format!("release.yml does not run on {other}")),
    };
    let kind = if publish || channel == "prod" {
        channel
    } else {
        "none"
    };
    Ok(Plan {
        channel,
        publish,
        pre: publish && channel == "dev",
        kind,
    })
}

fn required(name: &str) -> Result<String, String> {
    match std::env::var(name) {
        Ok(value) if !value.is_empty() => Ok(value),
        _ => Err(format!("{name}: parameter null or not set")),
    }
}

/// Reads EVENT_NAME, REF_NAME, INPUT_CHANNEL, INPUT_PUBLISH; returns the exit code.
pub fn run() -> Result<i32> {
    let inputs = (|| Ok::<_, String>((required("EVENT_NAME")?, required("REF_NAME")?)))();
    let plan = inputs.and_then(|(event, reference)| {
        decide(
            &event,
            &reference,
            std::env::var("INPUT_CHANNEL").ok().as_deref(),
            std::env::var("INPUT_PUBLISH").ok().as_deref(),
        )
    });
    match plan {
        Ok(plan) => {
            super::write_outputs(&[
                format!("channel={}", plan.channel),
                format!("publish={}", plan.publish),
                format!("pre={}", plan.pre),
                format!("kind={}", plan.kind),
            ])?;
            Ok(0)
        }
        Err(message) => {
            eprintln!("{message}");
            Ok(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(channel: &'static str, publish: bool, pre: bool, kind: &'static str) -> Plan {
        Plan {
            channel,
            publish,
            pre,
            kind,
        }
    }

    #[test]
    fn a_push_to_development_is_a_dev_pre_release() {
        assert_eq!(
            decide("push", "development", None, None),
            Ok(plan("dev", true, true, "dev"))
        );
    }

    #[test]
    fn a_push_to_main_is_a_prod_release() {
        assert_eq!(
            decide("push", "main", Some(""), Some("")),
            Ok(plan("prod", true, false, "prod"))
        );
    }

    #[test]
    fn a_push_to_another_branch_is_refused() {
        let error = decide("push", "feature/x", None, None).unwrap_err();
        assert_eq!(
            error,
            "release.yml publishes from development and main only, not feature/x"
        );
    }

    #[test]
    fn a_dispatch_defaults_to_a_dev_build_that_publishes_nothing() {
        assert_eq!(
            decide("workflow_dispatch", "feature/x", None, None),
            Ok(plan("dev", false, false, "none"))
        );
        assert_eq!(
            decide("workflow_dispatch", "main", Some(""), Some("")),
            Ok(plan("dev", false, false, "none"))
        );
    }

    #[test]
    fn a_dispatch_of_prod_without_publishing_still_has_a_prod_kind() {
        assert_eq!(
            decide(
                "workflow_dispatch",
                "feature/x",
                Some("prod"),
                Some("false")
            ),
            Ok(plan("prod", false, false, "prod"))
        );
    }

    #[test]
    fn a_dispatch_publishes_only_from_the_branch_of_its_channel() {
        let cases = [
            ("dev", "development", true),
            ("dev", "main", false),
            ("dev", "other", false),
            ("prod", "main", true),
            ("prod", "development", false),
            ("prod", "other", false),
        ];
        for (channel, reference, ok) in cases {
            let got = decide("workflow_dispatch", reference, Some(channel), Some("true"));
            assert_eq!(got.is_ok(), ok, "{channel} on {reference}");
            if let Ok(found) = got {
                assert!(found.publish);
                assert_eq!(found.pre, channel == "dev");
                assert_eq!(found.kind, channel);
            }
        }
        assert_eq!(
            decide("workflow_dispatch", "main", Some("dev"), Some("true")).unwrap_err(),
            "publish needs the dev channel on development or the prod channel on main (this run: dev on main)"
        );
    }

    #[test]
    fn bad_values_are_refused() {
        assert_eq!(
            decide("workflow_dispatch", "main", Some("beta"), None).unwrap_err(),
            "channel must be dev or prod, not beta"
        );
        assert_eq!(
            decide("workflow_dispatch", "main", None, Some("yes")).unwrap_err(),
            "publish must be true or false, not yes"
        );
        assert_eq!(
            decide("schedule", "main", None, None).unwrap_err(),
            "release.yml does not run on schedule"
        );
    }
}
