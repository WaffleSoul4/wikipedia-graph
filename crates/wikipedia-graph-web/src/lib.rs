#[cfg(target_arch = "wasm32")]
mod wasm {
    use eframe::wasm_bindgen::{self, prelude::*};
    use wikipedia_egui_graph::WikipediaGraphApp;

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
                Box::new(|cc| Ok(Box::new(WikipediaGraphApp::new(cc)))),
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
}
