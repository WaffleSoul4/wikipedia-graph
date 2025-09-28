use crate::client::WikipediaClientCommon;

use super::WikipediaClientConfig;
use isolang::Language;
use std::fmt::Display;
use thiserror::Error;
use ureq::Agent;
use url::Url;

type InnerClient = ureq::Agent;

type HttpErrorInner = ureq::Error;

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("Error with HTTP backend: {0}")]
    Backend(#[from] HttpErrorInner),
    #[error("Error parsing URL: {0}")]
    UrlParseError(#[from] url::ParseError),
    #[error("URL was malformed: '{0}'")]
    BadUri(String),
    #[error("Page not found at URL")]
    PageNotFound,
    #[error("Failed to get page before timout '{0}'")]
    Timeout(ureq::Timeout),
}

pub struct WikipediaClient {
    client: InnerClient,
    language: Language,
    headers: http::HeaderMap,
}

impl WikipediaClient {
    pub fn get<T: Display>(&self, pathinfo: T) -> Result<String, HttpError> {
        let url: Url = self.url_from_pathinfo(pathinfo)?;

        log::info!("Loading page from url '{}'", &url);

        let mut request = self.client.get(url.to_string());

        for (name, value) in self.headers.clone() {
            request = request.header(name.expect("All headers must have a name"), value);
        }

        let response = request
            .call()
            .and_then(|body| body.into_body().read_to_string())
            .map_err(|err| match err {
                ureq::Error::StatusCode(404) => HttpError::PageNotFound,
                ureq::Error::BadUri(uri) => HttpError::BadUri(uri),
                ureq::Error::Timeout(timeout) => HttpError::Timeout(timeout),
                _ => HttpError::Backend(err),
            });

        Ok(response?)
    }

    pub fn from_config(config: WikipediaClientConfig) -> Result<Self, HttpError> {
        let builder = ureq::config::Config::builder()
            .max_redirects(super::CLIENT_REDIRECTS)
            .timeout_global(config.timeout);

        let client = Agent::new_with_config(builder.build());

        Ok(WikipediaClient {
            client,
            language: config.language,
            headers: config.headers,
        })
    }
}

impl WikipediaClientCommon for WikipediaClient {
    fn language(&self) -> isolang::Language {
        self.language
    }
}

impl Default for WikipediaClient {
    fn default() -> Self {
        Self::from_config(WikipediaClientConfig::default())
            .expect("Default ureq client is not valid")
    }
}
