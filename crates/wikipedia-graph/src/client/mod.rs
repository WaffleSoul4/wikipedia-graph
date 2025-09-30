use std::{str::FromStr, time::Duration};
use thiserror::Error;
use ureq::http::{HeaderMap, HeaderName, HeaderValue};
use url::Url;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
pub mod wasm;
#[cfg(target_arch = "wasm32")]
pub use wasm::*;

const CLIENT_REDIRECTS: u32 = 2;

pub struct WikipediaClientConfig {
    timeout: Option<Duration>,
    // Only non defaults
    headers: ureq::http::HeaderMap<HeaderValue>,
    language: isolang::Language,
}

const USER_AGENT: &'static str = concat!(
    std::env!("CARGO_PKG_NAME"),
    "/",
    std::env!("CARGO_PKG_VERSION")
);

#[derive(Error, Debug)]
pub enum HeaderError {
    #[error("{0}")]
    InvalidHeaderName(#[from] http::header::InvalidHeaderName),
    #[error("{0}")]
    InvalidHeaderValue(#[from] http::header::InvalidHeaderValue),
    #[error("{0}")]
    HeaderMapMaxSizeReached(#[from] http::header::MaxSizeReached),
}

impl WikipediaClientConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn user_agent(self, user_agent: impl std::fmt::Display) -> Result<Self, HeaderError> {
        self.add_header(http::header::USER_AGENT, user_agent)
    }

    pub fn timeout(self, timeout: Option<Duration>) -> Self {
        Self { timeout, ..self }
    }

    pub fn language(self, language: isolang::Language) -> Self {
        Self { language, ..self }
    }

    pub fn add_header(
        mut self,
        name: impl std::fmt::Display,
        value: impl std::fmt::Display,
    ) -> Result<Self, HeaderError> {
        self.headers.try_insert(
            HeaderName::from_str(name.to_string().as_str())?,
            HeaderValue::from_str(value.to_string().as_str())?,
        )?;

        Ok(self)
    }
}

impl Default for WikipediaClientConfig {
    fn default() -> Self {
        WikipediaClientConfig {
            language: isolang::Language::from_639_1("en").expect("Language 'en' does not exist"),
            timeout: Some(Duration::from_secs(5)),
            headers: HeaderMap::new(),
        }
        .user_agent(USER_AGENT)
        .expect("Default headers are invalid")
    }
}

pub trait WikipediaClientCommon {
    fn language(&self) -> isolang::Language;

    fn base_url(&self) -> Result<Url, crate::page::LanguageInvalidError> {
        crate::page::wikipedia_base_with_language(self.language())
    }

    fn url_from_pathinfo<T: std::fmt::Display>(&self, pathinfo: T) -> Result<Url, url::ParseError> {
        let pathinfo = pathinfo.to_string();

        let base_url = self.base_url().expect("Selected language is invalid");

        if pathinfo.eq("Special:Random") {
            return Ok(
                Url::parse(format!("{}special:random", base_url.to_string()).as_str())
                    .expect("Random URL is not valid"),
            );
        }

        self.base_url()
            .expect("Selected language is invalid")
            .join(pathinfo.as_str())
    }
}

#[cfg(test)]
mod test {
    use crate::{WikipediaClient, WikipediaClientConfig};
    #[test]
    fn default_client_config_is_valid() {
        let config = WikipediaClientConfig::default();

        WikipediaClient::from_config(config).expect("Default configuration is invalid");
    }
    mod language {
        use isolang::Language;

        use crate::page::wikipedia_base_with_language;

        const TEST_LANGUAGES: [(&str, &str); 20] = [
            ("ar", "Arabic"),
            ("da", "Danish"),
            ("de", "German"),
            ("el", "Greek"),
            ("en", "English"),
            ("eo", "Esperanto"),
            ("es", "Spanish"),
            ("fr", "French"),
            ("he", "Hebrew"),
            ("hi", "Hindi"),
            ("is", "Icelandic"),
            ("it", "Italian"),
            ("ko", "Korean"),
            ("la", "Latin"),
            ("nv", "Navajo"),
            ("pt", "Portuguese"),
            ("ru", "Russian"),
            ("sv", "Swedish"),
            ("to", "Tongan"),
            ("zh", "Chinese"),
        ];

        #[test]
        fn languages_are_valid() {
            for (iso, name) in TEST_LANGUAGES {
                let url = wikipedia_base_with_language(
                    Language::from_639_1(iso)
                        .expect(format!("Iso code '{iso}' is invalid").as_str()),
                )
                .expect(format!("Language '{name}' has no iso 639-1 code").as_str());
                if !url.host_str().map_or(false, |host| {
                    host.starts_with(iso) && host.ends_with("wikipedia.org")
                }) {
                    panic!("Url does not start with the respective iso code")
                }
            }
        }
    }
}
