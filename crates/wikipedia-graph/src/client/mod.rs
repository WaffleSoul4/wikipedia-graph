mod client;
pub use client::*;

use crate::wikimedia_languages::WikiLanguage;
use http::{HeaderMap, HeaderName, HeaderValue};
use std::{str::FromStr, time::Duration};
use thiserror::Error;
use url::Url;

/// Amount of redirects the client accepts
const CLIENT_REDIRECTS: u32 = 2;

/// The configuration for a WikipediaClient
///
/// This currently stores the same information as the client itself, but may be useful later
pub struct WikipediaClientConfig {
    timeout: Option<Duration>,
    // Only non-default headers
    headers: HeaderMap<HeaderValue>,
    language: WikiLanguage,
}

/// The default user agent
const USER_AGENT: &'static str = concat!(
    std::env!("CARGO_PKG_NAME"),
    "/",
    std::env!("CARGO_PKG_VERSION")
);

/// A wrapper around all possible header errors from the http crate
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

    /// Sets the user agent of the client
    ///
    /// This is recommended if you're planning on making many requests
    ///
    /// The default value is "wikipedia-graph/{current version}"
    pub fn user_agent(self, user_agent: impl std::fmt::Display) -> Result<Self, HeaderError> {
        self.add_header(http::header::USER_AGENT, user_agent)
    }

    /// Sets the request timeout, or how long to wait for a request before returning an error
    ///
    /// The default value is 5 seconds
    pub fn timeout(self, timeout: Option<Duration>) -> Self {
        Self { timeout, ..self }
    }

    /// Sets the language of the request
    ///
    /// For example, the 'Waffle' page becomes into the URL 'https://{wikipedia language code}.wikipedia.org/wiki/Waffle'
    ///
    /// The default value is temporarily English
    pub fn language(self, language: WikiLanguage) -> Self {
        Self { language, ..self }
    }

    /// Adds a header to the request
    ///
    /// This is helpful for CORS authentication and probably a few other things
    ///
    /// # Errors
    ///
    /// This method fails whenever the passed headers fail to be parsed
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
        let mut headers = HeaderMap::new();

        headers.append(
            http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_str("*").unwrap(),
        );

        WikipediaClientConfig {
            language: WikiLanguage::from_code("en").expect("Language 'en' does not exist"),
            timeout: Some(Duration::from_secs(5)),
            headers,
        }
        .user_agent(USER_AGENT)
        .expect("Default headers are invalid")
    }
}

/// A trait that handles language and URL configuration for clients
///
/// This is currently useless, since there is only one client
trait WikipediaClientCommon {
    fn language(&self) -> WikiLanguage;

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
    mod language {
        use crate::WikiLanguage;

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
            for (code, name) in TEST_LANGUAGES {
                let url = wikipedia_base_with_language(
                    WikiLanguage::from_code(code).expect(format!("Wikipedia code '{code}' is invalid").as_str()),
                )
                .expect(format!("Language '{name}' has no wikipedia code").as_str());
                if !url.host_str().map_or(false, |host| {
                    host.starts_with(code) && host.ends_with("wikipedia.org")
                }) {
                    panic!("Url does not start with the correct wikipedia language code")
                }
            }
        }
    }
}
