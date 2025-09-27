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

use crate::page::LanguageInvalidError;

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

// Some langs don't have an iso 639-1
pub fn wikipedia_base_with_language(
    language: isolang::Language,
) -> Result<Url, LanguageInvalidError> {
    Ok(Url::parse(
        format!(
            "https://{}.wikipedia.org/wiki/",
            language.to_639_1().ok_or(LanguageInvalidError)?
        )
        .as_str(),
    )
    .expect(
        format!(
            "Wikipedia URL with language '{:?}' parsing failed",
            language
        )
        .as_str(),
    ))
}
