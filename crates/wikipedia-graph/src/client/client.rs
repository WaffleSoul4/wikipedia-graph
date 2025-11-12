use super::WikipediaClientConfig;
use crate::WikiLanguage;
use crate::client::WikipediaClientCommon;
use crate::page::{LanguageInvalidError, WikipediaBody, WikipediaUrlType};
use ehttp::{Headers, Request, Response};
use http::StatusCode;
use serde::ser;
use serde_json::Value;
use std::fmt::Display;
#[allow(unused_imports)] // For wasm stuff
use std::sync::{Arc, Mutex};
use thiserror::Error;
use url::Url;
#[allow(unused_imports)] // For wasm stuff
use web_time::{Duration, Instant};

pub trait Callback: 'static + Fn() + Send + Clone {}

impl<T: 'static + Fn() + Send + Clone> Callback for T {}

/// The Errors that may occur with the HTTP client
#[derive(Debug, Error)]
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
    /// Inner error type for redirecting, not really an error
    #[doc(hidden)]
    #[error("This is solely an inner type used for detecting redirects")]
    Redirect(String),
    /// The request returned an unknown response code
    #[error("Unknown response code: '{0}'")]
    Unknown(u16),
    /// The client failed to get the information from the request thread
    #[error("Failed to sync data containing response")]
    SyncError,
    /// The client failed to deserialise the response
    #[error("Failed to deserialise response: {0}")]
    DeserialisationError(#[from] serde_json::Error),
}

/// A client used for getting Wikipedia pages
pub struct WikipediaClient {
    timeout: Option<Duration>,
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
    /// # Errors
    ///
    /// The method fails if the request fails
    /// There can only be two reasons for this:
    ///     - A client side error
    ///     - Wikipedia is down
    pub fn get_api_base(&self) -> Result<(), HttpError> {
        self.get_request(
            Request::get(format!("https://en.wikipedia.org/w/api.php?origin=*")),
            || {},
        )
        .map(|_| ())
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
    pub fn get_api_base_callback(&self, callback: impl Callback) -> Result<(), HttpError> {
        self.get_request(
            Request::get(format!("https://en.wikipedia.org/w/api.php?origin=*")),
            callback,
        )
        .map(|_| ())
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
        callback_extra: impl Callback,
    ) -> Result<String, HttpError> {
        log::info!("Loading page from url '{}'", &request.url);

        let mut redirects = crate::client::CLIENT_REDIRECTS;

        let mut request = request.clone();

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

        'redirect_loop: while redirects != 0 {
            let callback = callback_extra.clone();

            #[cfg(not(target_arch = "wasm32"))]
            let (response_writer, response_reader) = std::sync::mpsc::channel();

            #[cfg(target_arch = "wasm32")]
            let response_store: Arc<Mutex<Option<Result<String, HttpError>>>> =
                Arc::new(Mutex::new(None));

            #[cfg(target_arch = "wasm32")]
            let response_store_clone = response_store.clone();

            ehttp::fetch(request.clone(), move |response| {
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

                callback();

                log::info!("Sending response... ");

                #[cfg(not(target_arch = "wasm32"))]
                if let Err(e) = response_writer.send(Some(response_processed)) {
                    panic!("Failed to send response to app: {e}")
                }

                #[cfg(target_arch = "wasm32")]
                match response_store_clone.lock() {
                    Ok(mut t) => *t = Some(response_processed),
                    Err(e) => panic!("Failed to send response to app: {e}"),
                }
            });

            redirects -= 1;

            #[cfg(not(target_arch = "wasm32"))]
            loop {
                match response_reader.recv_timeout(self.timeout.unwrap_or(Duration::from_hours(16)))
                {
                    Ok(mut t) => match t.take() {
                        Some(response) => {
                            log::info!("Response recieved");

                            if let Err(HttpError::Redirect(url)) = response {
                                request.url = url;
                                continue 'redirect_loop;
                            }

                            return response;
                        }
                        None => return Err(HttpError::NoPageBody),
                    },
                    Err(_) => return Err(HttpError::Timeout),
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                let poll_start = Instant::now();

                loop {
                    while poll_start.elapsed().as_millis() % 1000 != 0 {
                        log::info!("Blocking!!")
                    }

                    log::info!("{:?}", response_store);

                    match response_store.lock() {
                        Ok(mut t) => match t.take() {
                            Some(response) => {
                                log::info!("Response recieved");

                                if let Err(HttpError::Redirect(url)) = response {
                                    request.url = url;
                                    continue 'redirect_loop;
                                }

                                return response;
                            }
                            None if Instant::now().duration_since(poll_start)
                                > self.timeout.unwrap_or(Duration::from_hours(24)) =>
                            {
                                log::warn!("Request timed out");
                                return Err(HttpError::Timeout);
                            }
                            None => {}
                        },
                        Err(_) => return Err(HttpError::SyncError),
                    }
                }
            }
        }

        Err(HttpError::TooManyRedirects)
    }

    /// Get the wikipedia page at the specified pathinfo
    ///
    /// # Errors
    ///
    /// This method fails if the http request failed
    pub fn get<T: Display>(&self, pathinfo: T) -> Result<WikipediaBody, HttpError> {
        self.get_callback(pathinfo, || {})
    }

    /// Get the wikipedia page at the specified pathinfo
    ///
    /// Executes the given callback upon request completion
    ///
    /// # Errors
    ///
    /// This method fails if the http request failed
    pub fn get_callback<T: Display>(
        &self,
        pathinfo: T,
        callback: impl Callback,
    ) -> Result<WikipediaBody, HttpError> {
        let request = self.request_from_pathinfo(pathinfo, self.url_type)?;

        let response = self.get_request(request, callback);

        match self.url_type {
            WikipediaUrlType::LinksApi => {
                response.and_then(|response| {
                    serde_json::from_str::<Value>(&response)
                        .map(|response| WikipediaBody::Links(response))
                        .map_err(|err| err.into())
                })
            },
            WikipediaUrlType::RawApi => {
                response.and_then(|response| {
                    serde_json::from_str::<Value>(&response)
                        .map(|response| WikipediaBody::Normal(response))
                        .map_err(|err| err.into())
                })
            },
            WikipediaUrlType::Normal => {
                Err(HttpError::DeserialisationError(
                    <serde_json::Error as ser::Error>::custom(
                        "Can't deserialize links from the Normal Request Type",
                    ),
                )) //TODO: Please fix this
            }
        }
    }

    /// Returns the title of a random page using the Wikimedia API
    ///
    /// # Errors
    ///
    /// This method fails if the request failed
    pub fn random_title(&self) -> Result<String, HttpError> {
        self.random_title_callback(|| {})
    }

    /// returns the title of a random page using the Wikimedia API
    ///
    /// Executes the given callback upon request completion
    ///
    /// # Errors
    ///
    /// This method fails if the request failed
    pub fn random_title_callback(&self, callback: impl Callback) -> Result<String, HttpError> {
        let mut base_url = WikipediaUrlType::LinksApi.base_url(self.language)?;

        base_url.set_query(Some(
            "action=query&format=json&list=random&rnnamespace=0&rnlimit=1&origin=*",
        ));

        let request = Request::get(base_url);

        let deserialized: Value =
            serde_json::from_str(self.get_request(request, callback)?.as_str())?;

        deserialized // Completely readable code!!
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
    }

    /// Create a [WikipediaClient] from a [WikipediaClientConfig]
    pub fn from_config(config: WikipediaClientConfig) -> Self {
        WikipediaClient {
            timeout: config.timeout,
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
