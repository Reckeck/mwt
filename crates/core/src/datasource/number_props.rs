//! Properties of the number reader.

use proptest::prelude::*;

use super::number::parse_number;
use crate::Amount;
use crate::model::datasource::NumberFormat;
use crate::testkit::props::amount;

fn format() -> impl Strategy<Value = NumberFormat> {
    (
        any::<bool>(),
        "[ a-z%$]{0,3}",
        prop::sample::select(vec![1u32, 100, 1000]),
    )
        .prop_map(|(decimal_comma, strip, multiplier)| NumberFormat {
            decimal_comma,
            multiplier: Amount::from(multiplier),
            strip,
        })
}

proptest! {
    #[test]
    fn parse_number_never_panics(text in any::<String>(), format in format()) {
        if let Some(number) = parse_number(&text, &format) {
            prop_assert!(number.within_input_bound());
        }
    }

    #[test]
    fn parse_number_reads_back_a_formatted_number(
        value in amount(-1_000_000_000_000..=1_000_000_000_000, 2),
        decimal_comma in any::<bool>(),
        prefix in prop::sample::select(vec!["", "price ", "$", "total: "]),
        suffix in prop::sample::select(vec!["", " %", " rub", "$"]),
    ) {
        let digits = value.to_string();
        let digits = if decimal_comma { digits.replace('.', ",") } else { digits };
        let format = NumberFormat { decimal_comma, multiplier: Amount::from(1), strip: String::new() };
        let parsed = parse_number(&format!("{prefix}{digits}{suffix}"), &format);
        prop_assert_eq!(parsed, Some(value));
    }
}
