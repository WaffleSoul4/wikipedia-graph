use crate::wikimedia_languages::WikiLanguage;
use itertools::Itertools;
use regex::Regex;
use serde_json::Value;
use thiserror::Error;
use url::Url;

#[cfg(feature = "client")]
use crate::client::{HttpError, WikipediaClient};

/// A struct representing the location of a Wikipedia page and its body
#[derive(Clone, Debug)]
pub struct WikipediaPage {
    // This is called 'pathinfo' it's the part of the url after the /
    pathinfo: String,
    body: Option<WikipediaBody>,
}

/// An error that may occur when a language has no iso 639-1 representation
#[derive(Error, Debug)]
#[error("Language has no valid iso 639-1 representation")]
pub struct LanguageInvalidError;

/// An error that may occur when the pathinfo of a page cannot be seperated from its body
#[derive(Debug, Error)]
#[error(
    "Failed to parse pathinfo from body, pathinfo can only be parsed from WikipediaBody::Links"
)]
pub struct PathinfoParseError;

/// An error that may occur when parsing directly from a wikipedia URL
#[derive(Debug, Error)]
pub enum WikipediaUrlError {
    /// The host of the URL does not lead to wikipedia.org
    #[error("URL host is not the wikipedia domain")]
    InvalidHost,
    /// The path of the URL does not lead to a wiki
    ///
    /// For example: 'wikipedia.org/wiki/Waffle vs. wikipedia.org/Waffle (I don't know if this actually happens)
    #[error("URL path does not lead to a wiki")]
    InvalidPath,

    /// The Url cannot be parsed for an unknown reason
    #[error("Invalid URL: '{0}'")]
    InvalidURL(#[from] url::ParseError),
}

#[derive(Clone, Debug)]
pub enum WikipediaBody {
    Normal(serde_json::Value),
    Links(serde_json::Value),
}

impl WikipediaBody {
    /// A list of page titles that won't be included in linked pages
    const FILTERED_PAGES: [&str; 1] = [
        "Wayback Machine", // Almost all sources are linked to through the wayback machine
    ];

    pub fn normal_from_text(text: &str) -> Result<WikipediaBody, serde_json::Error> {
        serde_json::from_str(text).map(|val| WikipediaBody::Normal(val))
    }

    pub fn links_from_text(text: &str) -> Result<WikipediaBody, serde_json::Error> {
        serde_json::from_str(text).map(|val| WikipediaBody::Links(val))
    }

    /// Print the body as a string
    ///
    /// Output is either JSON or HTML
    pub fn to_string(self) -> String {
        match self {
            Self::Normal(t) => t.to_string(),
            Self::Links(t) => t.to_string(),
        }
    }

    pub fn get_pathinfo(&self) -> Result<String, PathinfoParseError> {
        match self {
            WikipediaBody::Normal(_) => Err(PathinfoParseError),
            WikipediaBody::Links(links) => Self::get_pathinfo_from_links(&links),
        }
    }

    fn get_pathinfo_from_links(data: &serde_json::Value) -> Result<String, PathinfoParseError> {
        data.get("query")
            .and_then(|query| query.get("pages")?.as_object()?.iter().next())
            .map(|(_, value)| value)
            .and_then(|value| value.get("title")?.as_str())
            .map(|title| dbg!(title.to_string()))
            .ok_or(PathinfoParseError)
    }

    pub fn get_linked_pages(&self) -> Vec<WikipediaPage> {
        match self {
            WikipediaBody::Normal(t) => Self::get_linked_pages_from_page_text(t),
            WikipediaBody::Links(t) => Self::get_linked_pages_from_links(t),
        }
    }

    fn get_linked_pages_from_links(data: &serde_json::Value) -> Vec<WikipediaPage> {
        data.get("query")
            .and_then(|query| query.get("pages")?.as_object()?.iter().next())
            .map(|(_, value)| value)
            .and_then(|data| data.get("links")?.as_array())
            .map(|links| {
                links
                    .iter()
                    .filter_map(|link| {
                        Some(WikipediaPage::from_title(link.get("title")?.as_str()?))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn get_linked_pages_from_page_text(value: &Value) -> Vec<WikipediaPage> {
        let page_text = match value
            .get("parse")
            .and_then(|parse| parse.get("wikitext"))
            .and_then(|wikitext| wikitext.as_object()?.iter().next()?.1.as_str())
        {
            Some(t) => t,
            None => return Vec::new(),
        };

        let regex = Regex::new(
            r#"\[\[([a-zA-Z0-9 \(\)]+)(?:[|][a-zA-Z0-9 \(\)]+)?\]\]"#, // Don't ask
        )
        .expect("Failed to compile regex to find linked pages");

        regex
            .captures_iter(&page_text)
            .map(|capture| capture.extract::<1>())
            .unique_by(|capture_data| capture_data.1[0])
            .filter(|capture_data| {
                Self::FILTERED_PAGES
                    .iter()
                    .all(|page| !capture_data.0.contains(page))
            })
            .map(|capture_data| WikipediaPage::from_title(capture_data.1[0]))
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WikipediaUrlType {
    Normal,
    RawApi,
    LinksApi,
}

impl WikipediaUrlType {
    pub fn base_url(&self, language: WikiLanguage) -> Result<Url, LanguageInvalidError> {
        Ok(match self {
            Self::Normal => Url::parse(
                format!(
                    "https://{}.wikipedia.org/wiki/",
                    language.as_code_wiki().ok_or(LanguageInvalidError)?
                )
                .as_str(),
            )
            .expect(
                format!(
                    "Base Wikipedia URL with language '{:?}' parsing failed",
                    language
                )
                .as_str(),
            ),
            Self::LinksApi | Self::RawApi => Url::parse(
                format!(
                    "https://{}.wikipedia.org/w/api.php",
                    language.as_code_wiki().ok_or(LanguageInvalidError)?
                )
                .as_str(),
            )
            .expect(
                format!(
                    "Base Wikipedia API URL with language '{:?}' parsing failed",
                    language
                )
                .as_str(),
            ),
        })
    }

    pub fn url_with(
        &self,
        language: WikiLanguage,
        pathinfo: &String,
    ) -> Result<Url, LanguageInvalidError> {
        match self {
            WikipediaUrlType::Normal => Ok(self.base_url(language)?.join(&pathinfo).expect(
                format!(
                    "Wikipedia URL for '{}' with language '{:?}' parsing failed",
                    pathinfo, language
                )
                .as_str(),
            )),
            WikipediaUrlType::RawApi => {
                let mut url = self.base_url(language)?;
                url.set_query(Some(
                    format!(
                        "origin=*&action=parse&prop=wikitext&format=json&page={}",
                        pathinfo
                    )
                    .as_str(),
                ));
                Ok(url)
            }
            WikipediaUrlType::LinksApi => {
                let mut url = self.base_url(language)?;
                url.set_query(Some(
                    format!(
                        "action=query&format=json&prop=links&pllimit=500&origin=*&titles={}",
                        pathinfo
                    )
                    .as_str(),
                ));
                Ok(url)
            }
        }
    }
}

fn verify_url(url: &Url) -> Result<(), WikipediaUrlError> {
    let host_str = url.host_str().unwrap_or("");

    if !host_str.ends_with("wikipedia.org") || !(url.scheme() == "http" || url.scheme() == "https")
    {
        return Err(WikipediaUrlError::InvalidHost);
    }

    let mut path = url.path_segments().ok_or(WikipediaUrlError::InvalidPath)?;

    if !path.next().map_or(false, |path| path == "wiki") {
        return Err(WikipediaUrlError::InvalidPath);
    }

    Ok(())
}

impl WikipediaPage {
    /// Manually set the page body of a wikipedia page
    ///
    /// This is helpful for loading pages from places other than wikipedia.org
    pub fn set_page_body(&mut self, data: WikipediaBody) -> &mut Self {
        let pathinfo_new = data.get_pathinfo();

        self.body = Some(data);

        match pathinfo_new {
            Ok(pathinfo) => self.pathinfo = pathinfo,
            Err(e) => log::error!("{e}"),
        }

        self
    }

    /// Check if the page text is loaded
    pub fn is_page_text_loaded(&self) -> bool {
        self.body.is_some()
    }

    /// Get the pathinfo of the wikipedia page
    pub fn pathinfo(&self) -> &String {
        &self.pathinfo
    }

    /// Get the url of the wikipedia page with a certain language
    pub fn url_with_lang(&self, language: WikiLanguage) -> Result<Url, LanguageInvalidError> {
        WikipediaUrlType::Normal.url_with(language, &self.pathinfo)
    }

    /// Create a new WikipediaPage from the title
    ///
    /// For example: `Waffle` to the the Waffle page
    pub fn from_title(title: impl Into<String>) -> Self {
        let title: String = title.into();

        WikipediaPage {
            pathinfo: title.replace(" ", "_"),
            body: None,
        }
    }

    /// Try to create a new WikipediaPage from a path
    ///
    /// For example: `/wiki/Waffle/` to get the Waffle page
    pub fn try_from_path(path: impl Into<String>) -> Result<Self, WikipediaUrlError> {
        let base = Url::parse("https://wikipedia.org/wiki/")
            .expect("Url 'https://wikipedia.org/wiki/' is invalid");

        let joined = base.join(path.into().as_str())?;

        verify_url(&joined)?;

        let title = joined
            .path_segments()
            .and_then(|segments| segments.last())
            .ok_or(WikipediaUrlError::InvalidPath)?;

        Ok(Self::from_title(title))
    }

    /// Try to create a new WikipediaPage from a URL
    ///
    /// For example: `https://wikipedia.org/wiki/Waffle` to get the Waffle page
    pub fn try_from_url(url: Url) -> Result<Self, WikipediaUrlError> {
        verify_url(&url)?;

        let mut base = Url::parse(url.origin().ascii_serialization().as_str())
            .expect("Origin should always be a valid URL");

        base.set_path("/wiki/");

        base.make_relative(&url)
            .ok_or(WikipediaUrlError::InvalidPath)
            .map(|val| WikipediaPage {
                pathinfo: val,
                body: None,
            })
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "client")] {
            // Does not load the body into memory

            /// Get the text of the page without loading into memory, or retrieve it from memory
            ///
            /// *This method requires the `client` feature*
            ///
            /// # Errors
            ///
            /// This method fails if the request for the page data fails
            pub fn get_page_body(&self, client: &WikipediaClient) -> Result<WikipediaBody, HttpError> {
                match &self.body {
                    Some(t) => Ok(t.clone()),
                    _ => client.get(self.pathinfo.clone()),
                }
            }

            /// Get a random and unloaded page from the wikimedia API
            ///
            /// *This method requires the `client` feature*
            ///
            /// # Errors
            ///
            /// This method fails if the request for a random page fails
            pub fn random(client: &WikipediaClient) -> Result<Self, HttpError> {
                client.random_title().map(|result| Self::from_title(result))
            }

            // Load the page text from the internet no matter what

            /// Load the page text and store it in memory
            ///
            /// *This method requires the `client` feature*
            ///
            /// # Errors
            ///
            /// This method fails if the request for the page data fails
            pub fn force_load_page_text(
                &mut self,
                client: &WikipediaClient,
            ) -> Result<&mut Self, HttpError> {
                let page_body = client.get(self.pathinfo.clone())?;

                let pathinfo_new = page_body.get_pathinfo();

                self.body = Some(page_body);

                match pathinfo_new {
                    Ok(pathinfo) => self.pathinfo = pathinfo,
                    Err(e) => log::error!("{e}"),
                }

                Ok(self)
            }

            /// Load the page text if it is not already stored in memory
            ///
            /// *This method requires the `client` feature*
            ///
            /// # Errors
            ///
            /// This method fails if the request for the page data fails
            pub fn load_page_text(&mut self, client: &WikipediaClient) -> Result<&mut Self, HttpError> {
                let text = self.get_page_body(client)?;

                self.body = Some(text);

                Ok(self)
            }
        }
    }

    /// Remove any page text from memory
    pub fn unload_body(&mut self) -> &mut Self {
        self.body = None;

        self
    }

    // All the 'try_...' functions mean is that they don't make any requests

    /// Get the page text if it it loaded
    pub fn try_get_page_body(&self) -> Option<WikipediaBody> {
        self.body.clone()
    }

    /// Give a best guess at the title of the page
    pub fn title(&self) -> String {
        capitalize(url_encor::decode(self.pathinfo.replace("_", " ").as_str()))
    }

    /// Get all the pages that this page links to if the page text is loaded
    pub fn try_get_linked_pages(&self) -> Option<Vec<WikipediaPage>> {
        self.body.as_ref().map(|body| body.get_linked_pages())
    }
}

fn capitalize(input: String) -> String {
    let mut capitialize: bool = true;

    input
        .trim()
        .replace("_", " ")
        .chars()
        .map(|char| match (char.is_whitespace(), capitialize) {
            (true, true) => '*',
            (false, true) => {
                capitialize = false;
                if let Some(uppercase) = char.to_uppercase().next() {
                    uppercase
                } else {
                    char
                }
            }
            (true, false) => {
                capitialize = true;
                char
            }
            _ => char,
        })
        .collect::<String>()
        .replace("*", "")
}
