mod client;
pub use client::*;

use crate::{
    page::{WikipediaLanguageInvalidError, WikipediaUrlType},
    wikimedia_languages::WikiLanguage,
};
use http::{HeaderMap, HeaderName, HeaderValue};
use std::{collections::HashMap, str::FromStr};
use thiserror::Error;
use url::Url;

/// The configuration for a WikipediaClient
///
/// This currently stores the same information as the client itself, but may be useful later
pub struct WikipediaClientConfig {
    // Only non-default headers
    headers: HeaderMap<HeaderValue>,
    language: WikiLanguage,
    url_type: WikipediaUrlType,
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
    /// The header name was invalid
    #[error("{0}")]
    InvalidHeaderName(#[from] http::header::InvalidHeaderName),
    /// The header value was invalid
    #[error("{0}")]
    InvalidHeaderValue(#[from] http::header::InvalidHeaderValue),
    #[error("{0}")]
    /// The maximum amount of headers was reached
    HeaderMapMaxSizeReached(#[from] http::header::MaxSizeReached),
}

impl WikipediaClientConfig {
    /// Create a new instance of [WikipediaClientConfig] with set values
    ///
    /// # Errors
    ///
    /// This method fails whenever invalid headers are provided
    pub fn new(headers: HashMap<&str, &str>, language: WikiLanguage) -> Result<Self, HeaderError> {
        let without_headers = Self::default().language(language);

        headers
            .iter()
            .try_fold(without_headers, |without_headers, (name, value)| {
                without_headers.add_header(name, value)
            })
    }

    /// Sets the user agent of the client
    ///
    /// This is recommended if you're planning on making many requests
    ///
    /// The default value is "wikipedia-graph/{current version}"
    pub fn user_agent(self, user_agent: impl std::fmt::Display) -> Result<Self, HeaderError> {
        self.add_header(http::header::USER_AGENT, user_agent)
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

    /// Returns the headers of the client config
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }
}

impl Default for WikipediaClientConfig {
    fn default() -> Self {
        let headers = HeaderMap::new();

        WikipediaClientConfig {
            language: WikiLanguage::from_code("en").expect("Language 'en' does not exist"),
            headers,
            url_type: WikipediaUrlType::RawApi,
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

    fn url_from_pathinfo<T: std::fmt::Display>(
        &self,
        pathinfo: T,
        url_type: WikipediaUrlType,
    ) -> Result<Url, WikipediaLanguageInvalidError> {
        let pathinfo = pathinfo.to_string();

        url_type.url_with(self.language(), &pathinfo)
    }
}

#[cfg(test)]
mod test {
    mod language {
        use crate::WikiLanguage;

        use crate::page::WikipediaUrlType;

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
                let url = WikipediaUrlType::Basic
                    .base_url(
                        WikiLanguage::from_code(code)
                            .expect(format!("Wikipedia code '{code}' is invalid").as_str()),
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
