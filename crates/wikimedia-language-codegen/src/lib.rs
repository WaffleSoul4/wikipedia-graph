use std::fmt::Display;

use codegen::{Scope, Variant};
use serde_json::Value;

const LANGUAGE_ENUM_NAME: &'static str = "WikiLanguages";

pub struct LanguageData {
    code: String,
    name: String,
    local_name: String,
}

pub fn languages_from_sitematrix(site_matrix: &Value) -> Vec<LanguageData> {
    site_matrix
        .as_object()
        .expect("Failed to convert site matrix to an object")
        .iter()
        .filter_map(|(title, value)| {
            if title.parse::<u64>().is_err() {
                return None;
            }

            let code = value.get("code")?.as_str()?;
            let name = value.get("name")?.as_str()?;
            let local_name = value.get("localname")?.as_str()?;

            Some(LanguageData::new(code, name, local_name))
        })
        .collect()
}

pub fn languages_as_enum_code(languages: Vec<LanguageData>) -> Scope {
    // Enum
    let mut scope = Scope::new();

    let language_enum = scope.new_enum(LANGUAGE_ENUM_NAME);

    languages.iter().for_each(|language_data| {
        language_enum.push_variant(Variant::new(language_data.enum_variant_unqualified()));
    });

    // Impl

    let language_impl = scope.new_impl(LANGUAGE_ENUM_NAME);

    // as_code

    let as_code = language_impl.new_fn("as_code").arg_ref_self().vis("pub");

    let codes_arms = languages
        .iter()
        .map(LanguageData::code_match_arm)
        .map(|string| format!("    {string}"))
        .collect::<String>();

    let language_as_code = format!("match self {{\n{codes_arms}}}");

    as_code
        .line(language_as_code);

    // as_name

    let as_name = language_impl.new_fn("as_name").arg_ref_self().vis("pub");

    let names_arms = languages
        .iter()
        .map(LanguageData::name_match_arm)
        .map(|string| format!("    {string}"))
        .collect::<String>();

    let language_as_name = format!("match self {{\n{names_arms}}}");

    as_name
        .line(language_as_name);

    scope
}

impl LanguageData {
    pub fn new(code: impl Display, name: impl Display, local_name: impl Display) -> Self {
        LanguageData {
            code: code.to_string(),
            name: name.to_string(),
            local_name: local_name.to_string(),
        }
    }

    fn enum_variant(&self) -> String {
        format!("{LANGUAGE_ENUM_NAME}::{}", self.enum_variant_unqualified())
    }

    fn enum_variant_unqualified(&self) -> String {
        capitalize(self.local_name.clone())
            .replace(" ", "")
            .replace("(", "")
            .replace(")", "")
    }

    fn code_match_arm(&self) -> String {
        format!("{} => \"{}\",\n", self.enum_variant(), self.code)
    }

    fn name_match_arm(&self) -> String {
        format!("{} => \"{}\",\n", self.enum_variant(), self.name)
    }
}

pub fn site_matrix() -> Value {
    let mut api = mediawiki::api_sync::ApiSync::new("https://en.wikipedia.org/w/api.php").unwrap();

    api.set_user_agent("wikiepdia-language-getter/wikipedia-graph");

    let params = api.params_into(&[("action", "sitematrix"), ("format", "json")]);

    

    api.get_query_api_json(&params).expect("request failed")
        .get("sitematrix")
        .expect("Invalid response")
        .clone()
}

fn capitalize(input: String) -> String {
    let mut capitialize: bool = true;

    input
        .trim()
        .replace("_", " ")
        .chars()
        .map(|char| match (char.is_whitespace(), capitialize) {
            (true, true) => '*',
            (false, true) => {
                capitialize = false;
                if let Some(uppercase) = char.to_uppercase().next() {
                    uppercase
                } else {
                    char
                }
            }
            (true, false) => {
                capitialize = true;
                char
            }
            _ => char,
        })
        .collect::<String>()
        .replace("*", "")
}
