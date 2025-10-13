use std::{
    collections::{HashMap, VecDeque},
    fmt::Display,
};

use codegen::{Scope, Variant};
use regex::Regex;
use serde_json::Value;

const LANGUAGE_ENUM_NAME: &'static str = "WikiLanguages";

#[derive(PartialEq, Eq, Hash)]
pub enum WikimediaCode {
    Wikipedia,
    Wiktionary,
    Wikibooks,
    Wikinews,
    Wikiquote,
    Wikisource,
    Wikiversity,
    Wikivoayge,
}

impl WikimediaCode {
    fn from_str(str: &str) -> Option<Self> {
        match str {
            "wiki" => Some(Self::Wikipedia),
            "wiktionary" => Some(Self::Wiktionary),
            "wikinews" => Some(Self::Wikinews),
            "wikiquote" => Some(Self::Wikiquote),
            "wikisource" => Some(Self::Wikisource),
            "wikiversity" => Some(Self::Wikiversity),
            "wikivoyage" => Some(Self::Wikivoayge),
            _ => None,
        }
    }

    fn as_str(&self) -> &str {
        match self {
            WikimediaCode::Wikipedia => "wiki",
            WikimediaCode::Wiktionary => "wiktionary",
            WikimediaCode::Wikibooks => "wikibooks",
            WikimediaCode::Wikinews => "wikinews",
            WikimediaCode::Wikiquote => "wikiquote",
            WikimediaCode::Wikisource => "wikisource",
            WikimediaCode::Wikiversity => "wikiversity",
            WikimediaCode::Wikivoayge => "wikivoyage",
        }
    }
}

const WIKIPEDIA_CODE_VARIANTS: [WikimediaCode; 8] = [
    WikimediaCode::Wikipedia,
    WikimediaCode::Wiktionary,
    WikimediaCode::Wikibooks,
    WikimediaCode::Wikinews,
    WikimediaCode::Wikiquote,
    WikimediaCode::Wikisource,
    WikimediaCode::Wikiversity,
    WikimediaCode::Wikivoayge,
];

pub struct LanguageData {
    universal_code: String,
    name: String,
    local_name: String,
    codes: HashMap<WikimediaCode, String>,
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

            // Two dots after to confirm it doesn't also take the
            let code_from_url_regex = Regex::new(r#"https://([a-z]*)\..+"#)
                .expect("Failed to compile regex to get code from url");

            let mut codes = HashMap::new();

            value
                .get("site")?
                .as_array()?
                .iter()
                .filter_map(|site_data| {
                    let url = site_data.get("url")?.as_str()?;

                    let code = code_from_url_regex.captures(url)?.extract::<1>().1[0];

                    Some((
                        WikimediaCode::from_str(site_data.get("code")?.as_str()?)?,
                        String::from(code),
                    ))
                })
                .for_each(|(wiki_code, code)| {
                    codes.insert(wiki_code, code);
                });

            let code = value.get("code")?.as_str()?;
            let name = value.get("name")?.as_str()?;
            let local_name = value.get("localname")?.as_str()?;

            Some(LanguageData::new(code, name, local_name, codes))
        })
        .collect()
}

pub fn languages_as_enum_code(languages: Vec<LanguageData>) -> Scope {
    let mut local_names = VecDeque::new();

    let languages = languages
        .into_iter()
        .map(|mut language| {
            if local_names.contains(&language.local_name) {
                language.local_name = format!("{}_{}", language.local_name, language.universal_code)
            }

            local_names.push_back(language.local_name.clone());

            language
        })
        .collect::<Vec<LanguageData>>();

    // Enum
    let mut scope = Scope::new();

    let language_enum = scope.new_enum(LANGUAGE_ENUM_NAME).vis("pub");

    languages.iter().for_each(|language_data| {
        language_enum.push_variant(Variant::new(language_data.enum_variant_unqualified()));
    });

    // Impl

    let language_impl = scope.new_impl(LANGUAGE_ENUM_NAME);

    // as_code

    WIKIPEDIA_CODE_VARIANTS.iter().for_each(|variant| {
        let as_code = language_impl
            .new_fn(format!("as_code_{}", variant.as_str()).as_str())
            .arg_ref_self()
            .vis("pub")
            .ret("Option<&str>");

        let codes_arms = languages
            .iter()
            .filter_map(|language| language.option_code_match_arm(variant))
            .map(|string| format!("    {string}"))
            .collect::<String>();

        let language_as_code = format!("match self {{\n{codes_arms}    _ => None,\n}}");

        as_code.line(language_as_code);
    });

    // as_name

    let as_name = language_impl
        .new_fn("as_name")
        .arg_ref_self()
        .vis("pub")
        .ret("&str");

    let names_arms = languages
        .iter()
        .map(LanguageData::name_match_arm)
        .map(|string| format!("    {string}"))
        .collect::<String>();

    let language_as_name = format!("match self {{\n{names_arms}}}");

    as_name.line(language_as_name);

    scope
}

impl LanguageData {
    pub fn new(
        code: impl Display,
        name: impl Display,
        local_name: impl Display,
        codes: HashMap<WikimediaCode, String>,
    ) -> Self {
        LanguageData {
            universal_code: code.to_string(),
            name: name.to_string(),
            local_name: local_name.to_string(),
            codes,
        }
    }

    fn enum_variant(&self) -> String {
        format!("{LANGUAGE_ENUM_NAME}::{}", self.enum_variant_unqualified())
    }

    fn enum_variant_unqualified(&self) -> String {
        capitalize(self.local_name.clone())
            .chars()
            .filter(|char| char.is_ascii_alphanumeric())
            .collect() // Sorry languages 3:
    }

    fn code_match_arm(&self, wikimedia_code: &WikimediaCode) -> Option<String> {
        Some(format!("{} => \"{}\",\n", self.enum_variant(), self.codes.get(&wikimedia_code)?))
    }

    fn option_code_match_arm(&self, wikimedia_code: &WikimediaCode) -> Option<String> {
        Some(format!("{} => Some(\"{}\"),\n", self.enum_variant(), self.codes.get(&wikimedia_code)?))
    }

    fn name_match_arm(&self) -> String {
        format!("{} => \"{}\",\n", self.enum_variant(), self.name)
    }
}

pub fn site_matrix() -> Value {
    let mut api = mediawiki::api_sync::ApiSync::new("https://en.wikipedia.org/w/api.php").unwrap();

    api.set_user_agent("wikiepdia-language-getter/wikipedia-graph");

    let params = api.params_into(&[("action", "sitematrix"), ("format", "json")]);

    api.get_query_api_json(&params)
        .expect("request failed")
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
