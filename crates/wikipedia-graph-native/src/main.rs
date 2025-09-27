use eframe::{NativeOptions, run_native};
use wikipedia_egui_graph::{WikipediaGraphApp, builder::WikipediaGraphAppBuilder};
use wikipedia_graph::Language;

fn main() {
    pretty_env_logger::init();

    let mut args = std::env::args();

    args.next();

    run_native(
        "Wikipedia Graph",
        NativeOptions::default(),
        Box::new(move |_cc| {
            let mut app_builder = WikipediaGraphAppBuilder::default();

            if let Some(lang) = args.next() {
                if let Some(language) = Language::from_639_1(lang.as_str()) {
                    app_builder = app_builder.with_language(language)
                } else {
                    log::warn!("Language entered is invalid")
                }
            }

            Ok(Box::new(app_builder.build()))
        }),
    )
    .unwrap();
}
