#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! ## Wikipedia Graph
//! 
//! A tool compatible with your favorite graphing crates to make graphing Wikipedia a walk in the forest
//! 
//! - A versatile struct for managing Wikipedia pages
//! - A configurable client
//! - Complete WASM support (theoretically)
//! 
//! # Example
//! ```no_run
//! # fn main() -> Result<(), wikipedia-graph::HttpError>
//! let mut page = WikipediaPage::from_title("Waffle");
//! page.load_page_text();
//! 
//! println!("Page title: {}", page.title());
//! 
//! for page in page.try_get_linked_pages().unwrap() {
//!     println!("Connects to {}", page.title());
//! }
//! # }
//! ```

mod page;

cfg_if::cfg_if! {
    if #[cfg(feature = "client")] {
        mod client;
        mod graph;

        pub use client::WikipediaClient;

        pub use client::WikipediaClientConfig;

        pub use client::HttpError;
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "graphs")] {

        pub use graph::{WikipediaGraph, IndexType};
    }
}




pub use page::{WikipediaPage, WikipediaUrlError};

pub use url::Url;

pub use isolang::Language;

