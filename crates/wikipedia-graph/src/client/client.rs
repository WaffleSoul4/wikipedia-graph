use super::WikipediaClientConfig;
use crate::client::WikipediaClientCommon;
use crate::page::{LanguageInvalidError, WikipediaBody, WikipediaUrlType};
use crate::{WikiLanguage, WikipediaPage};
use ehttp::{Headers, Request, Response};
use http::StatusCode;
use serde_json::Value;
use std::fmt::Display;
#[allow(unused_imports)] // For wasm stuff
use std::sync::{Arc, Mutex};
use thiserror::Error;
use url::Url;
#[allow(unused_imports)] // For wasm stuff
use web_time::{Duration, Instant};

/// The Errors that may occur with the HTTP client
#[derive(Debug, Error, Clone)]
pub enum HttpError {
    /// An unknown error with the backend (ehttp)
    #[error("Error with HTTP backend: {0}")]
    Backend(String),
    /// The provided URL couldn't be parsed
    #[error("Error parsing URL: {0}")]
    UrlParseError(#[from] url::ParseError),
    /// A Url with the provided language couldn't be made
    #[error("Language Invalid: {0}")]
    LanguageInvalidError(#[from] LanguageInvalidError),
    /// The requested page could not be found
    #[error("Page not found at URL")]
    PageNotFound,
    /// The request timed out
    #[error("Failed to get page before timeout")]
    Timeout,
    /// The returned page has no body
    #[error("Failed to find page body")]
    NoPageBody,
    /// The amount of redirects exceeded [crate::client::CLIENT_REDIRECTS]
    #[error("Too many redirects")]
    TooManyRedirects,
    /// Tell the user to redirect
    #[error("Please redirect to {0}")]
    Redirect(String), // Sorry, I'm no longer in control of the redirects anymore
    /// The request returned an unknown response code
    #[error("Unknown response code: '{0}'")]
    Unknown(u16),
    /// The client failed to deserialise the response
    #[error("Failed to deserialise response: {0}")]
    DeserialisationError(String), // serde_json::Error doesn't implement Clone
}

/// A client used for getting Wikipedia pages
pub struct WikipediaClient {
    language: WikiLanguage,
    headers: http::HeaderMap,
    url_type: WikipediaUrlType,
}

impl WikipediaClient {
    /// Create an ehttp::Request from the pathinfo of a wikipedia page
    ///
    /// # Errors
    ///
    /// This method fails if the url with the specified pathinfo and language is invalid
    pub fn request_from_pathinfo<T: Display>(
        &self,
        pathinfo: T,
        url_type: WikipediaUrlType,
    ) -> Result<Request, LanguageInvalidError> {
        let url: Url = self.url_from_pathinfo(pathinfo, url_type)?;

        let mut request = Request::get(url);

        request.headers =
            self.headers
                .iter()
                .fold(Headers::new(&[]), |mut headers, (name, value)| {
                    headers.insert(
                        name.to_string(),
                        value
                            .to_str()
                            .expect("Failed to convert a header value to a string")
                            .to_string(),
                    );
                    headers
                });

        Ok(request)
    }

    /// Get the contents of the page 'https://en.wikipedia.org/w/api.php', can be used as a network test
    ///
    /// Executes the given callback upon request completion
    ///
    /// # Errors
    ///
    /// The method fails if the request fails
    /// There can only be two reasons for this:
    ///     - A client side error
    ///     - Wikipedia is down
    pub fn get_api_base(&self, callback: impl Fn(Result<(), HttpError>) + 'static + Send) {
        let callback =
            move |result: Result<String, HttpError>| callback(result.map(|_: String| ()));

        self.get_request(
            Request::get(format!("https://en.wikipedia.org/w/api.php?origin=*")),
            callback,
        )
    }

    fn parse_status_code(code: StatusCode, response: Response) -> Result<Response, HttpError> {
        if code.is_redirection() {
            if let Some(redirect_url) = response.headers.get(http::header::LOCATION.as_str()) {
                log::info!("Redirecting to {redirect_url}");

                return Err(HttpError::Redirect(redirect_url.to_string()));
            }
        }

        if code.as_u16() == 404 {
            return Err(HttpError::PageNotFound);
        }

        if code.is_success() {
            return Ok(response);
        }

        Err(HttpError::Unknown(response.status))
    }

    fn get_request(
        &self,
        request: Request,
        callback: impl Fn(Result<String, HttpError>) + Send + 'static,
    ) {
        log::info!("Loading page from url '{}'", &request.url);

        let mut request = request;

        request.headers = Headers {
            headers: self
                .headers
                .iter()
                .map(|(name, value)| {
                    (
                        name.to_string(),
                        value
                            .to_str()
                            .expect("Failed to convert a header value to string")
                            .to_string(),
                    )
                })
                .collect(),
        };

        ehttp::fetch(request, move |response| {
            let response_processed = response
                .map_err(|err| HttpError::Backend(err))
                .and_then(|response| match StatusCode::from_u16(response.status) {
                    Ok(code) => WikipediaClient::parse_status_code(code, response),
                    Err(_) => Err(HttpError::Unknown(response.status)),
                })
                .and_then(|response: Response| {
                    response
                        .text()
                        .map(|text| text.to_string())
                        .ok_or(HttpError::NoPageBody)
                });

            log::info!("Running callback... ");

            callback(response_processed);
        });
    }

    /// Get the wikipedia page at the specified pathinfo
    ///
    /// Executes the given callback upon request completion
    ///
    /// # Errors
    ///
    /// This method fails if the http request failed
    pub fn get<T: Display>(
        &self,
        pathinfo: T,
        callback: impl Fn(Result<WikipediaBody, HttpError>) + Send + 'static,
    ) -> Result<(), LanguageInvalidError> {
        let request = self.request_from_pathinfo(pathinfo, self.url_type)?;

        let url_type = self.url_type.clone();

        self.get_request(request, move |respone| {
            callback(respone.and_then(|body| {
                WikipediaBody::from_url_type(url_type, body)
                    .map_err(|err| HttpError::DeserialisationError(err.to_string()))
            }))
        });

        Ok(())
    }

    /// returns the title of a random page using the Wikimedia API
    ///
    /// Executes the given callback upon request completion
    ///
    /// # Errors
    ///
    /// This method fails if the request failed
    pub fn random_page(
        &self,
        callback: impl Fn(Result<WikipediaPage, HttpError>) + Send + 'static,
    ) -> Result<(), LanguageInvalidError> {
        let mut base_url = WikipediaUrlType::LinksApi.base_url(self.language)?;

        base_url.set_query(Some(
            "action=query&format=json&list=random&rnnamespace=0&rnlimit=1&origin=*",
        ));

        let request = Request::get(base_url);

        let callback = move |response: Result<String, HttpError>| {
            callback(response.and_then(|body| {
                serde_json::from_str::<Value>(body.as_str())
                    .map_err(|err| HttpError::DeserialisationError(err.to_string()))?
                    .get("query")
                    .and_then(|val| val.get("random"))
                    .and_then(|val| val.as_array())
                    .and_then(|data| {
                        data.get(0)
                            .and_then(|data| data.get("title"))
                            .and_then(|title| title.as_str())
                            .map(|title| title.to_string())
                    })
                    .ok_or(HttpError::NoPageBody)
                    .map(WikipediaPage::from_title)
            }));
        };

        self.get_request(request, callback);

        Ok(())
    }

    /// Create a [WikipediaClient] from a [WikipediaClientConfig]
    pub fn from_config(config: WikipediaClientConfig) -> Self {
        WikipediaClient {
            language: config.language,
            headers: config.headers,
            url_type: config.url_type,
        }
    }
}

impl WikipediaClientCommon for WikipediaClient {
    fn language(&self) -> WikiLanguage {
        self.language
    }
}

impl Default for WikipediaClient {
    fn default() -> Self {
        Self::from_config(WikipediaClientConfig::default())
    }
}
