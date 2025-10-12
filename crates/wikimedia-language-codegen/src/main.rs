use std::fmt::Display;

use serde_json::Value;

const LANGUAGE_ENUM_NAME: &'static str = "WikiLanguages";

fn main() {
    let site_matrix = site_matrix();

    let site_matrix = site_matrix
        .get("sitematrix")
        .expect("Failed to get sitematrix");

    let count = site_matrix
        .get("count")
        .expect("Failed to get page count")
        .as_u64()
        .unwrap();

    println!("Article Count: {count}");

    let languages = site_matrix
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
        .collect::<Vec<_>>();

    let enum_variants = languages
        .iter()
        .map(|language_data| {
            let mut enum_variant = language_data.enum_variant();
            enum_variant.push(',');
            enum_variant.push('\n');
            enum_variant
        })
        .collect::<String>();

    let enum_code = format!("pub enum {LANGUAGE_ENUM_NAME} {{\n{enum_variants}}}");

    let codes_arms = languages
        .iter()
        .map(LanguageData::code_match_arm)
        .collect::<String>();

    let language_as_code = format!("pub fn as_code(&self) -> &str {{\n{codes_arms}}}");

    let names_arms = languages
        .iter()
        .map(LanguageData::name_match_arm) // >:3
        .collect::<String>();

    let language_as_name = format!("pub fn as_name(&self) -> &str {{\n{names_arms}}}");

    println!("{language_as_name}");

    let code = format!(
        "
{enum_code}\n
\n 
impl {LANGUAGE_ENUM_NAME} {{\n
{language_as_code}\n
{language_as_name}
}}
        "
    );

    println!("{code}")
}

struct LanguageData {
    code: String,
    name: String,
    local_name: String,
}

impl LanguageData {
    fn new(code: impl Display, name: impl Display, local_name: impl Display) -> Self {
        LanguageData {
            code: code.to_string(),
            name: name.to_string(),
            local_name: local_name.to_string(),
        }
    }

    fn enum_variant(&self) -> String {
        capitalize(self.local_name.clone())
            .replace(" ", "")
            .replace("(", "")
            .replace(")", "")
    }

    fn code_match_arm(&self) -> String {
        format!(
            "{} => \"{}\",\n",
            self.enum_variant(),
            self.code
        )
    }

    fn name_match_arm(&self) -> String {
        format!(
            "{} => \"{}\",\n",
            self.enum_variant(),
            self.name
        )
    }
}

fn site_matrix() -> Value {
    let mut api = mediawiki::api_sync::ApiSync::new("https://en.wikipedia.org/w/api.php").unwrap();

    api.set_user_agent("wikiepdia-language-getter/wikipedia-graph");

    let params = api.params_into(&[("action", "sitematrix"), ("format", "json")]);

    api.get_query_api_json(&params).expect("request failed")
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
