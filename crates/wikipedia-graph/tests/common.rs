#![allow(unused)]

use std::io::Read;
use url::Url;
use wikipedia_graph::{WikipediaClient, WikipediaPage};

pub fn multekrem_page() -> WikipediaPage {
    let mut page = WikipediaPage::from_title("Multekrem");

    let page_text = std::fs::read_to_string(std::path::Path::new("tests/multekrem-page-text"))
        .expect("Failed to find multekrem page text at tests/multekrem-page-text");

    assert!(!page_text.is_empty(), "Failed to load multekrem page");

    page.set_page_body(
        wikipedia_graph::WikipediaBody::normal_from_text(&page_text)
            .expect("Failed to parse multekrem page"),
    );

    page
}

pub const NUM_LINKED_MULTEKREM_PAGES: usize = 10;

const LINKED_MULTEKREM_PAGES: [&'static str; NUM_LINKED_MULTEKREM_PAGES] = [
    "https://wikipedia.org/wiki/Norway",
    "https://wikipedia.org/wiki/Dessert",
    "https://wikipedia.org/wiki/Rubus_chamaemorus",
    "https://wikipedia.org/wiki/Whipped_cream",
    "https://wikipedia.org/wiki/Sugar",
    "https://wikipedia.org/wiki/Norwegian_cuisine",
    "https://wikipedia.org/wiki/Dessert",
    "https://wikipedia.org/wiki/Krumkake",
    "https://wikipedia.org/wiki/Kransekake",
    "https://wikipedia.org/wiki/List_of_Norwegian_desserts",
];

pub fn multekrem_pages_iter() -> impl Iterator<Item = WikipediaPage> {
    LINKED_MULTEKREM_PAGES
        .into_iter()
        .map(|url| WikipediaPage::try_from_url(Url::parse(url).unwrap()).unwrap())
}
