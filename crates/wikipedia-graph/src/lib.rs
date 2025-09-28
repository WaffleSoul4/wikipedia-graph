#![forbid(unsafe_code)]

cfg_if::cfg_if! {
    if #[cfg(feature = "client")] {
        mod client;

        pub use client::WikipediaClient;

        pub use client::WikipediaClientConfig;

        pub use client::HttpError;
    }
}

mod graph;

pub use graph::{Indexable, WikipediaGraph};

mod page;

pub use page::{WikipediaPage, WikipediaUrlError};

pub use url::Url;

pub use isolang::Language;
