use std::io::Read;
use url::Url;
use wikipedia_graph::{WikipediaClient, WikipediaPage};

pub fn multekrem_page() -> WikipediaPage {
    let mut page = WikipediaPage::from_title("Multekrem");

    let mut file = std::fs::File::open("tests/multekrem-page-text")
        .expect("Failed to find multekrem page text at tests/multekrem-page-text");

    // Just manually set the page_content to avoid making a request and accessing potentially variable information

    let buffer = &mut [];

    file.read(buffer)
        .expect("Failed to read from multekrem page at tests/multekrem-page-text");

    let buffer = buffer.to_vec();

    page.set_page_text(
        String::from_utf8(buffer)
            .expect("Failed to parse tests/multekrem-page-text as valid utf-8"),
    );

    page
}

const LINKED_MULTEKREM_PAGES: [&'static str; 8] = [
    "https://wikipedia.org/wiki/Dessert",
    "https://wikipedia.org/wiki/Norway",
    "https://wikipedia.org/wiki/Rubus_chamaemorus",
    "https://wikipedia.org/wiki/Whipped_cream",
    "https://wikipedia.org/wiki/Sugar",
    "https://wikipedia.org/wiki/Norwegian_cuisine",
    "https://wikipedia.org/wiki/Krumkake",
    "https://wikipedia.org/wiki/List_of_Norwegian_desserts",
];

pub fn multekrem_pages_iter() -> impl Iterator<Item = WikipediaPage> {
    LINKED_MULTEKREM_PAGES
        .into_iter()
        .map(|url| WikipediaPage::try_from_url(Url::parse(url).unwrap()).unwrap())
}
