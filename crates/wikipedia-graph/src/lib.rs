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
//! # use wikipedia_graph::{HttpError, WikipediaPage, WikipediaClient};
//! # use std::sync::mpsc::*;
//! # fn main() -> Result< (), Box<dyn std::error::Error>> {
//! let mut page = WikipediaPage::from_title("Waffle");
//! let client = WikipediaClient::default();
//!
//! let (response_sender, response_reciever) = channel::<Result<WikipediaPage, HttpError>>();
//!
//! page.load_page_text(&client, move |response| response_sender.send(response).expect("Failed to send response to main thread"));
//!
//! page = response_reciever.recv()??;
//!
//! println!("Page title: {}", page.title());
//!
//! for page in page.try_get_linked_pages().unwrap() {
//!     println!("Connects to {}", page.title());
//! }
//! # Ok(())
//! # }
//! ```

mod page;
mod wikimedia_languages {
    #![allow(missing_docs)]
    include!("generated/wikimedia_languages.rs");
}

cfg_if::cfg_if! {
    if #[cfg(feature = "client")] {
        mod client;
        mod graph;

        pub use client::HeaderError;

        pub use http::HeaderMap;

        pub use client::WikipediaClient;

        pub use client::WikipediaClientConfig;

        pub use client::HttpError;
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "graphs")] {

        pub use graph::{WikipediaGraph, DefaultIndexType};
    }
}

pub use page::{WikipediaPage, WikipediaUrlError};

pub use url::Url;

pub use wikimedia_languages::WikiLanguage;

pub use page::WikipediaBody;
