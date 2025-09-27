use super::WikipediaClientConfig;
use isolang::Language;
use itertools::concat;
use std::fmt::Display;
use thiserror::Error;
use ureq::Agent;
use url::{ParseError, Url};

const USER_AGENT: &str = "wikipdia-graph/0.1.2";

type InnerConfig = ureq::config::ConfigBuilder<ureq::typestate::AgentScope>;
type InnerClient = ureq::Agent;

type HttpErrorInner = ureq::Error;

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("Error with HTTP backend: {0}")]
    Backend(#[from] HttpErrorInner),
    #[error("Error parsing URL: {0}")]
    UrlParseError(#[from] url::ParseError),
}

pub struct WikipediaClient {
    client: InnerClient,
    language: Language,
    headers: http::HeaderMap,
}

// async fn fetch(client: &BaseClient, url: Url) -> Result<String, HttpError> {
//     println!("Hello from runtime");

//     Ok(client
//             .get(url)
//             .send()
//             .await?
//             .text()
//             .await?)
// }

impl WikipediaClient {
    pub fn get<T: Display>(&self, pathinfo: T) -> Result<String, HttpError> {
        let url: Url = self.url_from_pathinfo(pathinfo)?;

        log::info!("Loading page from url '{}'", &url);

        let mut request = self.client.get(url.to_string());

        for (name, value) in self.headers.clone() {
            request = request.header(name.expect("All headers must have a name"), value);
        }

        Ok(request
            .call()
            .and_then(|body| body.into_body().read_to_string())?)
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

    pub fn base_url(&self) -> Result<Url, super::LanguageInvalidError> {
        super::wikipedia_base_with_language(self.language)
    }

    pub fn url_from_pathinfo<T: Display>(&self, pathinfo: T) -> Result<Url, ParseError> {
        self.base_url()
            .expect("Selected language is invalid")
            .join(pathinfo.to_string().as_str())
    }
}

impl Default for WikipediaClient {
    fn default() -> Self {
        Self::from_config(WikipediaClientConfig::default())
            .expect("Default ureq client is not valid")
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

        use crate::client::wikipedia_base_with_language;

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
