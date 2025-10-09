#![cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures as _;
use wasm_log::Config;
use web_sys::HtmlCanvasElement;
use wikipedia_egui_graph::WikipediaGraphApp;

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    wasm_log::init(Config::default());

    log::info!("Starting app...");
    console_error_panic_hook::set_once();

    wasm_bindgen_futures::spawn_local(async {
        log::info!("Hello from async");
        let _ = run().await;
    });
    Ok(())
}

#[wasm_bindgen]
pub async fn run() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;

    log::info!("Window initialized");

    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("no document"))?;

    log::info!("Document initialized");

    let canvas = document
        .get_element_by_id("wiki-graph")
        .ok_or_else(|| JsValue::from_str("canvas with id 'the_canvas_id' not found"))?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| JsValue::from_str("failed to cast to HtmlCanvasElement"))?;

    let web_options = eframe::WebOptions::default();

    log::info!("Configuration options set");

    eframe::WebRunner::new()
        .start(
            canvas,
            web_options,
            Box::new(|cc| {
                log::info!("Creating app");
                Ok::<Box<dyn eframe::App>, _>(Box::new(
                    wikipedia_egui_graph::builder::WikipediaGraphAppBuilder::default().build(),
                ))
            }),
        )
        .await?;
    Ok(())
}
