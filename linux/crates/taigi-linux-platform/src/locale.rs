//! The language the UI strings are drawn in when the setting says `system`
//! (`DisplayLanguage::effective`). On Linux that is the POSIX message
//! locale — `LC_ALL`, else `LC_MESSAGES`, else `LANG` (the precedence
//! `setlocale(3)` applies) — reduced to the `language[-REGION]` tag the
//! desktop core's resolver reads (`DisplayLanguage::resolve_automatic`
//! splits on `-` / `_` and looks at the first part). `C` and `POSIX` carry
//! no language and resolve to Hanji, the product's own default.

use std::ffi::OsString;

pub fn system_locale() -> String {
    system_locale_with(|name| std::env::var_os(name))
}

pub fn system_locale_with(env: impl Fn(&str) -> Option<OsString>) -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|name| env(name))
        .map(|value| value.to_string_lossy().into_owned())
        .find(|value| !value.is_empty())
        .map(|value| tag_of(&value))
        .unwrap_or_default()
}

/// `zh_TW.UTF-8@foo` → `zh-TW`; `C` / `POSIX` → empty.
fn tag_of(locale: &str) -> String {
    let base = locale.split(['.', '@']).next().unwrap_or("");
    if base == "C" || base == "POSIX" {
        return String::new();
    }
    base.replace('_', "-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let map: HashMap<String, OsString> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), OsString::from(value)))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn lc_all_wins_then_lc_messages_then_lang() {
        assert_eq!(
            system_locale_with(env(&[
                ("LANG", "ja_JP.UTF-8"),
                ("LC_MESSAGES", "en_US.UTF-8")
            ])),
            "en-US"
        );
        assert_eq!(
            system_locale_with(env(&[("LC_ALL", "zh_TW.UTF-8"), ("LANG", "en_US.UTF-8")])),
            "zh-TW"
        );
        assert_eq!(system_locale_with(env(&[("LANG", "ja_JP.UTF-8")])), "ja-JP");
    }

    #[test]
    fn an_empty_variable_is_skipped_and_c_means_no_language() {
        assert_eq!(
            system_locale_with(env(&[("LC_ALL", ""), ("LANG", "en_GB.UTF-8")])),
            "en-GB"
        );
        assert_eq!(system_locale_with(env(&[("LANG", "C.UTF-8")])), "");
        assert_eq!(system_locale_with(env(&[("LANG", "POSIX")])), "");
        assert_eq!(system_locale_with(env(&[])), "");
    }
}
