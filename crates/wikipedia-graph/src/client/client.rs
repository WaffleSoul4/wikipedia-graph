use super::WikipediaClientConfig;
use crate::client::WikipediaClientCommon;
use ehttp::{Headers, Request, Response};
use http::StatusCode;
use isolang::Language;
use std::{
    fmt::Display, sync::{Arc, Mutex}, thread, time::{Duration, Instant}
};
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("Error with HTTP backend: {0}")]
    Backend(String),
    #[error("Error parsing URL: {0}")]
    UrlParseError(#[from] url::ParseError),
    #[error("URL was malformed: '{0}'")]
    BadUri(String),
    #[error("Page not found at URL")]
    PageNotFound,
    #[error("Failed to get page before timeout")]
    Timeout,
    #[error("Failed to find page body")]
    NoPageBody,
    #[error("Too many redirects")]
    TooManyRedirects,
    #[doc(hidden)]
    #[error("This is solely an inner type used for detecting redirects")]
    Redirect(String),
    #[error("Unknown response code: '{0}'")]
    Unknown(u16),
    #[error("Failed to send data containing response")]
    SendError,
}

pub struct WikipediaClient {
    timeout: Option<Duration>,
    language: Language,
    headers: http::HeaderMap,
}

impl WikipediaClient {
    pub fn get<T: Display>(&self, pathinfo: T) -> Result<String, HttpError> {
        let url: Url = self.url_from_pathinfo(pathinfo)?;

        log::info!("Loading page from url '{}'", &url);

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

        let mut redirects = crate::client::CLIENT_REDIRECTS;

        'redirect_loop: while redirects != 0 {
            let (response_writer, response_reader) = std::sync::mpsc::channel();

            ehttp::fetch(request.clone(), move |response| {
                if let Err(e) = response_writer.send(Some(
                    response
                        .map_err(|err| HttpError::Backend(err))
                        .and_then(|response| match StatusCode::from_u16(response.status) {
                            Ok(code) => {
                                if code.is_redirection() {
                                    if let Some(redirect_url) =
                                        response.headers.get(http::header::LOCATION.as_str())
                                    {
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
                            Err(_) => Err(HttpError::Unknown(response.status)),
                        })
                        .and_then(|response: Response| {
                            response.text()
                                .map(|text| text.to_string())
                                .ok_or(HttpError::NoPageBody)
                        }),
                )) {
                    panic!("Failed to send response to app: {e}")
                }

            });

            redirects -= 1;

            loop {
                match response_reader.recv_timeout(self.timeout.unwrap_or(Duration::from_hours(16))) {
                    Ok(mut t) => match t.take() {
                        Some(response) => {
                            if let Err(HttpError::Redirect(url)) = response {
                                request.url = url;
                                continue 'redirect_loop;
                            }

                            return response;
                        }
                        None => {
                            return Err(HttpError::NoPageBody)
                        }
                    },
                    Err(_) => {
                        return Err(HttpError::Timeout)
                    }
                }
            }
        }

        Err(HttpError::TooManyRedirects)
    }

    pub fn from_config(config: WikipediaClientConfig) -> Result<Self, HttpError> {
        Ok(WikipediaClient {
            timeout: config.timeout,
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
