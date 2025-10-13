use crate::wikimedia_languages::WikiLanguage;
use itertools::Itertools;
use regex::Regex;
use thiserror::Error;
use url::Url;

#[cfg(feature = "client")]
use crate::client::{HttpError, WikipediaClient};

/// A struct representing the location of a Wikipedia page and its body
#[derive(Clone, Debug)]
pub struct WikipediaPage {
    // This is called 'pathinfo' it's the part of the url after the /
    pathinfo: String,
    page_text: Option<String>,
}

/// An error that may occur when a language has no iso 639-1 representation
#[derive(Error, Debug)]
#[error("Language has no valid iso 639-1 representation")]
pub struct LanguageInvalidError;

/// An error that may occur when the pathinfo of a page cannot be seperated from its body
#[derive(Debug, Error)]
#[error("Failed to parse pathinfo from body")]
pub struct PathinfoParseError;

/// An error that may occur when parsing directly from a wikipedia URL
#[derive(Debug, Error)]
pub enum WikipediaUrlError {
    #[error("URL host is not the wikipedia domain")]
    InvalidHost,
    // For example: https://en.wikipedia.org/wiki/Waffle vs. https://en.wikipedia.org/Waffle (Haven't actually been able to find a page like this yet)
    #[error("URL path does not lead to a wiki")]
    InvalidPath,
    #[error("Invalid URL: '{0}'")]
    InvalidURL(#[from] url::ParseError),
}

// Some langs don't have an iso 639-1
pub fn wikipedia_base_with_language(language: WikiLanguage) -> Result<Url, LanguageInvalidError> {
    Ok(Url::parse(
        format!(
            "https://{}.wikipedia.org/wiki/",
            language.as_code_wiki().ok_or(LanguageInvalidError)?
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
    /// A list of page titles that won't be included in linked pages
    const FILTERED_PAGES: [&str; 1] = [
        "Wayback Machine", // Almost all sources are linked to through the wayback machine
    ];

    /// Manually set the page text of a wikipedia page
    ///
    /// This is helpful for loading pages from places other than wikipedia.org
    pub fn set_page_text(&mut self, data: String) -> &mut Self {
        let pathinfo_new = Self::get_pathinfo_from_page_text(&data);

        self.page_text = Some(data);

        match pathinfo_new {
            Ok(pathinfo) => self.pathinfo = pathinfo,
            Err(e) => log::error!("{e}"),
        }

        self
    }

    /// Check if the page text is loaded
    pub fn is_page_text_loaded(&self) -> bool {
        self.page_text.is_some()
    }

    /// Create a special random Wikipedia page
    ///
    /// This requires a client to get the page and then update the pathinfo accordingly
    pub fn random(client: &WikipediaClient) -> Result<Self, HttpError> {
        let mut page = WikipediaPage::from_title("Special:Random");

        page.load_page_text(client)?;

        if let Err(e) = page.update_pathinfo_with_page_text(
            &page.try_get_page_text().expect("Failed to get page text"),
        ) {
            log::error!("{e}");
        };

        Ok(page)
    }

    /// Get the pathinfo of the wikipedia page
    pub fn pathinfo(&self) -> &String {
        &self.pathinfo
    }

    /// Get the url of the wikipedia page with a certain language
    pub fn url_with_lang(&self, language: WikiLanguage) -> Result<Url, LanguageInvalidError> {
        let base = crate::page::wikipedia_base_with_language(language)?;
        match base.join(&self.pathinfo) {
            Ok(t) => Ok(t),
            Err(e) => panic!(
                "Failed to join base url '{}' and '{}': {e}",
                base.as_str(),
                self.pathinfo
            ),
        }
    }

    /// Create a new WikipediaPage from the title
    ///
    /// For example: `Waffle` to the the Waffle page
    pub fn from_title(title: impl Into<String>) -> Self {
        let title: String = title.into();

        WikipediaPage {
            pathinfo: title.replace(" ", "_"),
            page_text: None,
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
                page_text: None,
            })
    }

    fn update_pathinfo_with_page_text(
        &mut self,
        page_text: &String,
    ) -> Result<&mut Self, PathinfoParseError> {
        self.pathinfo = Self::get_pathinfo_from_page_text(&page_text)?;

        Ok(self)
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
            pub fn get_page_text(&self, client: &WikipediaClient) -> Result<String, HttpError> {
                match &self.page_text {
                    Some(t) => Ok(t.clone()),
                    _ => client.get(self.pathinfo.clone()),
                }
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
                let page_text = client.get(self.pathinfo.clone())?;

                let pathinfo_new = Self::get_pathinfo_from_page_text(&page_text);

                self.page_text = Some(page_text);

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
                let text = self.get_page_text(client)?;

                self.page_text = Some(text);

                Ok(self)
            }
        }
    }

    /// Remove any page text from memory
    pub fn unload_body(&mut self) -> &mut Self {
        self.page_text = None;

        self
    }

    // All the 'try_...' functions mean is that they don't make any requests

    /// Get the page text if it it loaded
    pub fn try_get_page_text(&self) -> Option<String> {
        self.page_text.clone()
    }

    /// Give a best guess at the title of the page
    pub fn title(&self) -> String {
        capitalize(url_encor::decode(self.pathinfo.replace("_", " ").as_str()))
    }

    fn get_linked_pages_from_page_text(page_text: &String) -> Vec<WikipediaPage> {
        let regex = Regex::new(
            "<a href=\"(/wiki/[a-zA-Z_\\(\\)]+)\"(?: class=\"[a-zA-Z-_]\")? title=\"([a-zA-Z ]+)\"",
        )
        .expect("Failed to compile regex to find linked pages");

        regex
            .captures_iter(&page_text)
            .map(|capture| capture.extract::<2>())
            .unique_by(|capture_data| capture_data.1[0])
            .filter(|capture_data| {
                Self::FILTERED_PAGES
                    .iter()
                    .all(|page| !capture_data.0.contains(page))
            })
            .filter_map(|capture_data| {
                WikipediaPage::try_from_path(capture_data.1[0])
                    .ok()
                    .map(|mut page| {
                        page.pathinfo = capture_data.1[1].to_string().replace(" ", "_");

                        page
                    })
            })
            .collect()
    }

    fn get_pathinfo_from_page_text(page_text: &String) -> Result<String, PathinfoParseError> {
        let regex = Regex::new(
            r#"<link rel=\"canonical\" href=\"(https:\/\/[a-zA-Z\/\.]{2}\.wikipedia.org\/wiki\/(.*))\">"#,
        )
        .expect("Title regex failed to compile");

        Ok(page_text
            .lines()
            .filter(|l| l.contains("<link rel=\"canonical\""))
            .filter_map(|l| regex.captures(l))
            .next()
            .ok_or(PathinfoParseError)?
            .extract::<2>()
            .1[1]
            .to_string())
    }

    /// Get all the pages that this page links to if the page text is loaded
    pub fn try_get_linked_pages(&self) -> Option<Vec<WikipediaPage>> {
        if let Some(text) = &self.page_text {
            return Some(WikipediaPage::get_linked_pages_from_page_text(text));
        }

        None
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
