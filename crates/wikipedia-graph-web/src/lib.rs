#[cfg(target_arch = "wasm32")]
mod wasm {
    use eframe::wasm_bindgen::{self, prelude::*};
    use log::warn;
    use wikipedia_egui_graph::{
        WikiLanguage, WikipediaGraphApp, builder::WikipediaGraphAppBuilder,
    };

    #[derive(Clone)]
    #[wasm_bindgen] // Recursive, this is fixed in later versions
    pub struct WebHandle {
        runner: eframe::WebRunner,
    }

    #[wasm_bindgen]
    impl WebHandle {
        #[wasm_bindgen(constructor)]
        pub fn new() -> Self {
            wasm_log::init(wasm_log::Config::default());

            Self {
                runner: eframe::WebRunner::new(),
            }
        }

        /// Call this once from JavaScript to start your app.
        #[wasm_bindgen]
        pub async fn start(
            &self,
            canvas: web_sys::HtmlCanvasElement,
        ) -> Result<(), wasm_bindgen::JsValue> {
            log::info!("Starting wasm...");

            let mut builder = WikipediaGraphAppBuilder::default();

            if let Some(query) = web_sys::window().and_then(|window| decode_request(window))
                && let Some(language_index) = query.match_indices("lang=").next()
            {
                let language_code = &query[language_index.0 + 5..];

                match WikiLanguage::from_code(language_code) {
                    Some(language) => {
                        builder = builder.with_language(language);
                        log::info!("Language set to '{}'", language.as_name());
                    }
                    None => warn!("Failed to parse language code: {language_code}"),
                }
            }

            let app = self.runner.start(
                // web_sys::window()
                //     .expect("Failed to get window")
                //     .document()
                //     .expect("Failed to get document")
                //     .get_element_by_id("wiki-canvas")
                //     .expect("Failed to get wiki-canvas")
                //     .dyn_into::<web_sys::HtmlCanvasElement>()
                //     .expect("Failed to get canvas as a canvas"),
                canvas,
                eframe::WebOptions::default(),
                Box::new(move |cc| {
                    let mut app_builder = WikipediaGraphAppBuilder::default();

                    if let Some(query) = web_sys::window().and_then(|window| decode_request(window))
                        && let Some(language_index) = query.match_indices("lang=").next()
                    {
                        let language_code = &query[language_index.0 + 5..];

                        match WikiLanguage::from_code(language_code) {
                            Some(language) => {
                                app_builder = app_builder.with_language(language);
                                log::info!("Language set to '{}'", language.as_name());
                            }
                            None => warn!("Failed to parse language code: {language_code}"),
                        }
                    }

                    Ok(Box::new(app_builder.build()))
                }),
            );

            log::info!("Started runner... ");

            app.await
        }

        #[wasm_bindgen]
        pub fn destroy(&self) {
            self.runner.destroy();
        }

        /// The JavaScript can check whether or not your app has crashed:
        #[wasm_bindgen]
        pub fn has_panicked(&self) -> bool {
            self.runner.has_panicked()
        }

        #[wasm_bindgen]
        pub fn panic_message(&self) -> Option<String> {
            self.runner
                .panic_summary()
                .map(|s: eframe::web::PanicSummary| s.message())
        }

        #[wasm_bindgen]
        pub fn panic_callstack(&self) -> Option<String> {
            self.runner
                .panic_summary()
                .map(|s: eframe::web::PanicSummary| s.callstack())
        }
    }

    fn decode_request(window: web_sys::Window) -> Option<String> {
        match window.location().search() {
            Ok(s) => Some(s.trim_start_matches('?').to_owned()),
            _ => None,
        }
    }
}
