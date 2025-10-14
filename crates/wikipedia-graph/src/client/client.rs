use super::WikipediaClientConfig;
use crate::client::WikipediaClientCommon;
use ehttp::{Headers, Request, Response};
use http::StatusCode;
use crate::WikiLanguage;
use std::fmt::Display;
use thiserror::Error;
use url::Url;
use web_time::Duration;

/// The Errors that may occur with the HTTP client
#[derive(Debug, Error)]
pub enum HttpError {
    /// An unknown error with the backend (ehttp)
    #[error("Error with HTTP backend: {0}")]
    Backend(String),
    /// The provided URL couldn't be parsed
    #[error("Error parsing URL: {0}")]
    UrlParseError(#[from] url::ParseError),
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
}

/// A client used for getting Wikipedia pages
pub struct WikipediaClient {
    timeout: Option<Duration>,
    language: WikiLanguage,
    headers: http::HeaderMap,
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
    ) -> Result<Request, url::ParseError> {
        let url: Url = self.url_from_pathinfo(pathinfo)?;

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

    /// Get the wikipedia page at the specified pathinfo
    ///
    /// # Errors
    ///
    /// This method fails if the http request failed
    pub fn get<T: Display>(&self, pathinfo: T) -> Result<String, HttpError> {
        let mut request = self.request_from_pathinfo(pathinfo)?;

        log::info!("Loading page from url '{}'", &request.url);

        let mut redirects = crate::client::CLIENT_REDIRECTS;

        'redirect_loop: while redirects != 0 {
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
                    match response_store.lock() {
                        Ok(mut t) => match t.take() {
                            Some(response) => {
                                if let Err(HttpError::Redirect(url)) = response {
                                    request.url = url;
                                    continue 'redirect_loop;
                                }

                                return response;
                            }
                            None if Instant::now().duration_since(poll_start)
                                > self.timeout.unwrap_or(Duration::from_hours(24)) =>
                            {
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

    /// Create a [WikipediaClient] from a [WikipediaClientConfig]
    pub fn from_config(config: WikipediaClientConfig) -> Self {
        WikipediaClient {
            timeout: config.timeout,
            language: config.language,
            headers: config.headers,
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
