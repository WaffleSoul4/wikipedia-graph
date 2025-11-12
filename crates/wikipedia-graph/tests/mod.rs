mod common;
mod graphs;
use pretty_assertions::assert_eq;
use url::Url;
use wikipedia_graph::{WikiLanguage, WikipediaPage};

#[test]
fn page_creation() {
    let url = Url::parse("https://wikipedia.org/wiki/Waffle").unwrap();
    let page_1 = WikipediaPage::try_from_url(url).unwrap();
    let page_2 = WikipediaPage::try_from_path("/wiki/Waffle").unwrap();
    let page_3 = WikipediaPage::from_title("Waffle");
    assert_eq!(page_1.pathinfo(), page_2.pathinfo());
    assert_eq!(page_2.pathinfo(), page_3.pathinfo());
}

#[test]
fn linked_pages() {
    let page = common::multekrem_page();

    let linked_pages = page
        .try_get_linked_pages()
        .expect("Body failed to load (for some reason)");

    let multekrem_linked_pages = common::multekrem_pages_iter();

    linked_pages
        .into_iter()
        .zip(multekrem_linked_pages)
        .for_each(|(linked, known_linked)| {
            assert_eq!(
                linked.pathinfo().to_lowercase(),
                known_linked.pathinfo().to_lowercase()
            )
        }); // Better error message than itertools::eq
}

#[test]
fn get_title() {
    let page = common::multekrem_page();

    assert_eq!(page.title().as_str(), "Multekrem")
}

#[test]
fn get_url() {
    let page = common::multekrem_page();

    assert_eq!(
        page.url_with_lang(WikiLanguage::from_code("en").expect("Language code 'en' is invalid"))
            .expect("Multekrem URL is invalid")
            .as_str(),
        "https://en.wikipedia.org/wiki/Multekrem"
    );
}

#[test]
fn page_is_loaded() {
    let mut page = common::multekrem_page();

    assert!(page.is_page_text_loaded());

    page.unload_body();

    assert!(!page.is_page_text_loaded());
}
