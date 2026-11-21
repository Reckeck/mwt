//! `Option<LocalDate>` as an ISO `YYYY-MM-DD` string, for serde.

use serde::{Deserialize, Deserializer, Serializer};

use mwt_base::time::LocalDate;

pub(crate) fn serialize<S: Serializer>(
    date: &Option<LocalDate>,
    out: S,
) -> Result<S::Ok, S::Error> {
    match date {
        Some(date) => out.serialize_some(&date.to_string()),
        None => out.serialize_none(),
    }
}

pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
    input: D,
) -> Result<Option<LocalDate>, D::Error> {
    let text = Option::<String>::deserialize(input)?;
    text.map(|text| text.parse().map_err(serde::de::Error::custom))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct Holder {
        #[serde(default, with = "super")]
        date: Option<LocalDate>,
    }

    #[test]
    fn a_date_round_trips_as_iso_and_a_missing_one_is_none() {
        let holder = Holder {
            date: LocalDate::from_ymd_opt(2026, 3, 15),
        };
        let json = serde_json::to_string(&holder).unwrap();
        assert_eq!(json, r#"{"date":"2026-03-15"}"#);
        assert_eq!(serde_json::from_str::<Holder>(&json).unwrap(), holder);
        assert_eq!(
            serde_json::from_str::<Holder>("{}").unwrap(),
            Holder { date: None }
        );
    }
}
