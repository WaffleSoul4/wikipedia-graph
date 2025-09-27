mod common;
mod graphs;
use url::Url;
use wikipedia_graph::WikipediaPage;
#[test]
fn url_validation() {
    let url = Url::parse("https://wikipedia.org/wiki/Waffle").unwrap();
    let page_1 = dbg!(WikipediaPage::try_from_url(url).unwrap());
    let page_2 = WikipediaPage::from_path("/wiki/Waffle").unwrap();
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
        .for_each(|(linked, known_linked)| assert_eq!(linked.pathinfo(), known_linked.pathinfo()));
}
