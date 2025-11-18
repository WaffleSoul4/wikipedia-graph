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
#[derive(Error, Debug, Clone)]
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

/// The body of a Wikipedia page. The two current supported formats are the wikitext and links, both stored as JSON values.
#[derive(Clone, Debug)]
pub enum WikipediaBody {
    /// The (wikitext)[https://en.wikipedia.org/wiki/Help:Wikitext] of a page, stored in a thin layer of JSON
    /// 
    /// The wikitext JSON comes from this api call: <https://en.wikipedia.org/w/api.php?origin=*&action=parse&prop=wikitext&format=json&page=Waffle>
    WikiText(serde_json::Value),
    /// The links of a page, stored in a thin layer of JSON
    /// 
    /// The links JSON comes from this api call: <>
    Links(serde_json::Value),
}

impl WikipediaBody {
    /// A list of page titles that won't be included in linked pages
    const FILTERED_PAGES: [&str; 1] = [
        "Wayback Machine", // Almost all sources are linked to through the wayback machine
    ];

    const PAGE_TEXT_REGEX: &lazy_regex::Lazy<Regex> =
        lazy_regex::regex!(r#"\[\[([a-zA-Z0-9 \(\)]+)(?:[|][a-zA-Z0-9 \(\)]+)?\]\]"#);

    /// Serialize the JSON from a wikitext response and wrap it
    pub fn wikitext_from_text(text: &str) -> Result<WikipediaBody, serde_json::Error> {
        serde_json::from_str(text).map(|val| WikipediaBody::WikiText(val))
    }

    /// Serialize the JSON from a links response and wrap it
    pub fn links_from_text(text: &str) -> Result<WikipediaBody, serde_json::Error> {
        serde_json::from_str(text).map(|val| WikipediaBody::Links(val))
    }

    /// Print the body as a string
    ///
    /// Output is either JSON or HTML
    pub fn to_string(self) -> String {
        match self {
            Self::WikiText(t) => t.to_string(),
            Self::Links(t) => t.to_string(),
        }
    }

    /// Tries to create a [WikipediaBody] from text and the url type
    /// 
    /// # Errors
    /// 
    /// This method fails if the serialisation of the text fails or the type of the URL is Basic
    pub fn from_url_type(
        url_type: WikipediaUrlType,
        body: String,
    ) -> Result<WikipediaBody, serde_json::Error> {
        match url_type {
            WikipediaUrlType::LinksApi => {
                serde_json::from_str::<Value>(&body).map(|response| WikipediaBody::Links(response))
            }
            WikipediaUrlType::RawApi => serde_json::from_str::<Value>(&body)
                .map(|response| WikipediaBody::WikiText(response))
                .map_err(|err| err.into()),
            WikipediaUrlType::Basic => {
                Err(<serde_json::Error as serde::de::Error>::custom(
                    "Can't deserialize links from the Normal Request Type",
                )) //TODO: Please fix this
            }
        }
    }

    /// Get the pathinfo of a page from its body
    /// 
    /// # Errors
    /// 
    /// This method fails if the 'title' field is not available in the deserialised JSON
    pub fn get_pathinfo(&self) -> Result<String, PathinfoParseError> {
        match self {
            WikipediaBody::WikiText(_) => Err(PathinfoParseError),
            WikipediaBody::Links(links) => Self::get_pathinfo_from_links(&links),
        }
    }

    /// Get the pathinfo of a page stored with links
    /// 
    /// The pattern to access the title is `{query: {pages: {title: "Title"}}}`
    /// 
    /// # Errors
    /// 
    /// This method fails if the 'title' field is not available in the deserialized JSON
    pub fn get_pathinfo_from_links(data: &serde_json::Value) -> Result<String, PathinfoParseError> {
        data.get("query")
            .and_then(|query| query.get("pages")?.as_object()?.iter().next())
            .map(|(_, value)| value)
            .and_then(|value| value.get("title")?.as_str())
            .map(|title| title.to_string())
            .ok_or(PathinfoParseError)
    }

    /// Get the pathinfo of a page stored with wikitext
    /// 
    /// The structure to access the title is `{parse: {title: "Title"}}`
    /// 
    /// # Errors
    /// 
    /// This method fails if the 'title' field is not available in the deserialized JSON
    pub fn get_pathinfo_from_wikitext(data: &serde_json::Value) -> Result<String, PathinfoParseError> {
        data.get("parse")
            .and_then(|value| value.get("title")?.as_str())
            .map(|title| title.to_string())
            .ok_or(PathinfoParseError)
    }

    /// Get the linked pages of the body
    /// 
    /// Returns [None] if the recieved JSON is invalid
    pub fn get_linked_pages(&self) -> Option<Box<dyn Iterator<Item = WikipediaPage> + '_>> {
        match self {
            WikipediaBody::WikiText(t) => Some(Box::new(Self::get_linked_pages_from_wikitext(t)?)),
            WikipediaBody::Links(t) => Some(Box::new(Self::get_linked_pages_from_links(t)?)),
        }
    }

    /// Get the linked pages of a body in links format
    /// 
    /// The pattern to access the linked pages is `{query: {pages: {links: [{title: "Title"}]}}}``
    /// 
    /// Returns [None] if the recieved JSON is invalid
    pub fn get_linked_pages_from_links(
        value: &serde_json::Value,
    ) -> Option<impl Iterator<Item = WikipediaPage>> {
        value
            .get("query")
            .and_then(|query| query.get("pages")?.as_object()?.iter().next())
            .map(|(_, value)| value)
            .and_then(|data| data.get("links")?.as_array())
            .map(|links| {
                links.iter().filter_map(|link| {
                    Some(WikipediaPage::from_title(link.get("title")?.as_str()?))
                })
            })
    }

    /// Get the linked pages of a body in (wikitext)[https://en.wikipedia.org/wiki/Help:Wikitext] format
    /// 
    /// The pattern to access the wikitext pages is `{parse: {wikitext: "wikitext"}}`
    /// 
    /// Returns [None] if the recieved JSON is invalid
    pub fn get_linked_pages_from_wikitext(
        value: &Value,
    ) -> Option<impl Iterator<Item = WikipediaPage>> {
        let page_text = value
            .get("parse")
            .and_then(|parse| parse.get("wikitext"))
            .and_then(|wikitext| wikitext.as_object()?.iter().next()?.1.as_str())?;

        Some(
            Self::PAGE_TEXT_REGEX
                .captures_iter(&page_text)
                .map(|capture| capture.extract::<1>())
                .unique_by(|capture_data| capture_data.1[0])
                .filter(|capture_data| {
                    Self::FILTERED_PAGES
                        .iter()
                        .all(|page| !capture_data.0.contains(page))
                })
                .map(|capture_data| WikipediaPage::from_title(capture_data.1[0])),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WikipediaUrlType {
    Basic,
    RawApi,
    LinksApi,
}

impl WikipediaUrlType {
    pub fn base_url(&self, language: WikiLanguage) -> Result<Url, LanguageInvalidError> {
        Ok(match self {
            Self::Basic => Url::parse(
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
            WikipediaUrlType::Basic => Ok(self.base_url(language)?.join(&pathinfo).expect(
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

    /// Override the body of a wikipedia page
    pub fn with_body(self, body: WikipediaBody) -> Self {
        Self {
            body: Some(body),
            ..self
        }
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
        WikipediaUrlType::Basic.url_with(language, &self.pathinfo)
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

    /// Try to get the stored body of the page
    ///
    /// # Errors
    ///
    /// This method fails if the request for the page data fails
    pub fn try_get_body(&self) -> &Option<WikipediaBody> {
        &self.body
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "client")] {
            /// Get a random and unloaded page from the wikimedia API
            ///
            /// *This method requires the `client` feature*
            ///
            /// # Errors
            ///
            /// This method fails if the request for a random page fails
            pub fn random(client: &WikipediaClient, callback: impl Fn(Result<WikipediaPage, HttpError>) + Send + 'static) -> Result<(), LanguageInvalidError>  {
                client.random_page(callback)
            }

            /// Load the page text if it is not already stored in memory
            ///
            /// *This method requires the `client` feature*
            ///
            /// # Errors
            ///
            /// This method fails if the request for the page data fails
            pub fn load_page_text(&self, client: &WikipediaClient, callback: impl Fn(Result<Self, HttpError>) + Send + 'static) -> Result<(), LanguageInvalidError> {
                let title = self.title();

                client
                    .get(self.pathinfo.clone(), move |response| callback(response.map(|body| WikipediaPage::from_title(title.clone()).with_body(body))))
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
    pub fn try_get_linked_pages(&self) -> Option<Box<dyn Iterator<Item = WikipediaPage> + '_>> {
        self.body.as_ref().map(|body| body.get_linked_pages())?
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
