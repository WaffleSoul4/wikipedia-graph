use isolang::Language;
use itertools::Itertools;
use regex::Regex;
use thiserror::Error;
use url::Url;

#[cfg(feature = "client")]
use crate::client::{HttpError, WikipediaClient};

#[derive(Clone, Debug)]
pub struct WikipediaPage {
    // This is called 'pathinfo' it's the part of the url after the /
    pathinfo: String,
    page_data: WikipediaPageData,
}

#[derive(Debug, Clone)]
enum WikipediaPageData {
    FullText(String),
    Minimal { title: String },
    None,
}

#[derive(Error, Debug)]
#[error("Language has no valid iso 639-1 specification")]
pub struct LanguageInvalidError;

#[derive(Debug, Error)]
#[error("Failed to parse title from body")]
pub struct TitleParseError;

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
    const FILTERED_PAGES: [&str; 1] = ["Wayback Machine"];

    pub fn set_page_text(&mut self, data: String) -> &mut Self {
        self.page_data = WikipediaPageData::FullText(data);

        self
    }

    pub fn pathinfo(&self) -> &String {
        &self.pathinfo
    }

    pub fn random() -> Self {
        WikipediaPage {
            pathinfo: "Special:Random".to_string(),
            page_data: WikipediaPageData::None,
        }
    }

    pub fn url_with_lang(&self, language: Language) -> Result<Url, LanguageInvalidError> {
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

    pub fn from_title(title: impl Into<String>) -> Self {
        let title: String = title.into();

        WikipediaPage {
            pathinfo: title.replace(" ", "_"),
            page_data: WikipediaPageData::None,
        }
    }

    pub fn from_path(path: impl Into<String>) -> Result<Self, WikipediaUrlError> {
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

    pub fn try_from_url(url: Url) -> Result<Self, WikipediaUrlError> {
        verify_url(&url)?;

        let mut base = Url::parse(url.origin().ascii_serialization().as_str())
            .expect("Origin should always be a valid URL");

        base.set_path("/wiki/");

        base.make_relative(&url)
            .ok_or(WikipediaUrlError::InvalidPath)
            .map(|val| WikipediaPage {
                pathinfo: val,
                page_data: WikipediaPageData::None,
            })
    }

    cfg_if::cfg_if! {
        if #[cfg(feature = "client")] {
            // Does not load the body into memory
            pub fn get_page_text(&self, client: &WikipediaClient) -> Result<String, HttpError> {
                match &self.page_data {
                    WikipediaPageData::FullText(t) => Ok(t.clone()),
                    _ => client.get(self.pathinfo.clone()),
                }
            }

            // Load the page text from the internet no matter what
            pub fn force_load_page_text(
                &mut self,
                client: &WikipediaClient,
            ) -> Result<&mut Self, HttpError> {
                let page_text = client.get(self.pathinfo.clone())?;

                self.page_data = WikipediaPageData::FullText(page_text);

                Ok(self)
            }

            pub fn load_page_text(&mut self, client: &WikipediaClient) -> Result<&mut Self, HttpError> {
                self.page_data = WikipediaPageData::FullText(self.get_page_text(client)?);

                Ok(self)
            }

            pub fn minimize(&mut self, client: &WikipediaClient) -> Result<&mut Self, HttpError> {
                match &self.page_data {
                    WikipediaPageData::Minimal { title: _ } => {}
                    _ => {
                        let text = self.get_page_text(client)?;

                        let title =
                            WikipediaPage::get_title_from_page_text(&text).expect("Failed to find title");

                        self.page_data = WikipediaPageData::Minimal { title };
                    }
                }

                Ok(self)
            }
        }
    }

    // All the 'try_...' functions mean is that they don't make any requests
    pub fn try_get_page_text(&self) -> Option<String> {
        if let WikipediaPageData::FullText(text) = self.page_data.clone() {
            // This clone is technically uneccesary
            Some(text)
        } else {
            None
        }
    }

    pub fn try_get_title(&self) -> Option<Result<String, TitleParseError>> {
        match &self.page_data {
            WikipediaPageData::FullText(t) => Some(Self::get_title_from_page_text(t)),
            WikipediaPageData::Minimal { title } => Some(Ok(title.clone())),
            WikipediaPageData::None => None,
        }
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
                WikipediaPage::from_path(capture_data.1[0])
                    .ok()
                    .and_then(|mut page| {
                        page.page_data = WikipediaPageData::Minimal {
                            title: capture_data.1[1].to_string(),
                        };
                        Some(page)
                    })
            })
            .collect()
    }

    fn get_title_from_page_text(page_text: &String) -> Result<String, TitleParseError> {
        let regex = Regex::new(
            r#"<link rel=\"canonical\" href=\"(https:\/\/[a-zA-Z\/\.]{2}\.wikipedia.org\/wiki\/(.*))\">"#,
        )
        .expect("Title regex failed to compile");

        let title = page_text
            .lines()
            .filter(|l| l.contains("<link rel=\"canonical\""))
            .filter_map(|l| regex.captures(l))
            .next()
            .ok_or(TitleParseError)?
            .extract::<2>()
            .1[1]
            .to_string()
            .replace("_", " ");

        Ok(title)
    }

    pub fn try_get_linked_pages(&self) -> Option<Vec<WikipediaPage>> {
        if let WikipediaPageData::FullText(text) = &self.page_data {
            return Some(WikipediaPage::get_linked_pages_from_page_text(text));
        }

        None
    }
}
