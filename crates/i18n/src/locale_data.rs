//! The formatting conventions of the six languages.
//!
//! The values come from the Unicode CLDR (Unicode-3.0 license) for the locales `en`, `de`, `fr`,
//! `es`, `ja` and `uk`; every row has a hand-written expected row in the tests of `numbers` and
//! `datetime`. If a language is added, or the product needs month names, named time zones or
//! currencies, see research R4 (ICU4X is the planned replacement).

use crate::language::Language;

/// The order of the parts of a numeric date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DateOrder {
    /// Month, day, year.
    Mdy,
    /// Day, month, year.
    Dmy,
    /// Year, month, day.
    Ymd,
}

/// The conventions of one language.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LocaleData {
    /// The decimal separator.
    pub(crate) decimal: &'static str,
    /// The grouping separator.
    pub(crate) group: &'static str,
    /// The least number of digits before the first separator for grouping to apply; grouping
    /// happens only for integers with at least `3 + min_grouping` digits.
    pub(crate) min_grouping: usize,
    /// The order of the date parts.
    pub(crate) date_order: DateOrder,
    /// What separates the date parts.
    pub(crate) date_separator: &'static str,
    /// Whether day and month are written with two digits.
    pub(crate) pad_day_month: bool,
    /// Whether the clock has 24 hours (otherwise 12 hours with AM and PM).
    pub(crate) clock_24: bool,
    /// Whether the hour is written with two digits.
    pub(crate) pad_hour: bool,
    /// The marker for before noon (12-hour clock).
    pub(crate) am: &'static str,
    /// The marker for after noon (12-hour clock).
    pub(crate) pm: &'static str,
}

const EN: LocaleData = LocaleData {
    decimal: ".",
    group: ",",
    min_grouping: 1,
    date_order: DateOrder::Mdy,
    date_separator: "/",
    pad_day_month: false,
    clock_24: false,
    pad_hour: false,
    am: "AM",
    pm: "PM",
};

const DE: LocaleData = LocaleData {
    decimal: ",",
    group: ".",
    min_grouping: 1,
    date_order: DateOrder::Dmy,
    date_separator: ".",
    pad_day_month: true,
    clock_24: true,
    pad_hour: true,
    am: "",
    pm: "",
};

const FR: LocaleData = LocaleData {
    decimal: ",",
    group: "\u{202F}",
    min_grouping: 1,
    date_order: DateOrder::Dmy,
    date_separator: "/",
    pad_day_month: true,
    clock_24: true,
    pad_hour: true,
    am: "",
    pm: "",
};

const ES: LocaleData = LocaleData {
    decimal: ",",
    group: ".",
    min_grouping: 2,
    date_order: DateOrder::Dmy,
    date_separator: "/",
    pad_day_month: false,
    clock_24: true,
    pad_hour: false,
    am: "",
    pm: "",
};

const JA: LocaleData = LocaleData {
    decimal: ".",
    group: ",",
    min_grouping: 1,
    date_order: DateOrder::Ymd,
    date_separator: "/",
    pad_day_month: true,
    clock_24: true,
    pad_hour: false,
    am: "",
    pm: "",
};

const UK: LocaleData = LocaleData {
    decimal: ",",
    group: "\u{A0}",
    min_grouping: 1,
    date_order: DateOrder::Dmy,
    date_separator: ".",
    pad_day_month: true,
    clock_24: true,
    pad_hour: true,
    am: "",
    pm: "",
};

impl LocaleData {
    /// The conventions of `language`.
    pub(crate) const fn for_language(language: Language) -> &'static LocaleData {
        match language {
            Language::En => &EN,
            Language::De => &DE,
            Language::Fr => &FR,
            Language::Es => &ES,
            Language::Ja => &JA,
            Language::Uk => &UK,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_are_the_documented_ones() {
        // Hand-written from the CLDR data of each locale.
        let table: [(Language, &str, &str, usize, bool); 6] = [
            (Language::En, ".", ",", 1, false),
            (Language::De, ",", ".", 1, true),
            (Language::Fr, ",", "\u{202F}", 1, true),
            (Language::Es, ",", ".", 2, true),
            (Language::Ja, ".", ",", 1, true),
            (Language::Uk, ",", "\u{A0}", 1, true),
        ];
        for (language, decimal, group, min_grouping, clock_24) in table {
            let data = LocaleData::for_language(language);
            assert_eq!(data.decimal, decimal, "{language:?}");
            assert_eq!(data.group, group, "{language:?}");
            assert_eq!(data.min_grouping, min_grouping, "{language:?}");
            assert_eq!(data.clock_24, clock_24, "{language:?}");
        }
        assert_eq!(
            LocaleData::for_language(Language::Ja).date_order,
            DateOrder::Ymd
        );
        assert_eq!(
            LocaleData::for_language(Language::En).date_order,
            DateOrder::Mdy
        );
    }
}
