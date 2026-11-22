#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([480.0, 360.0])
            .with_title("BareBoot Workbench"),
        ..Default::default()
    };
    eframe::run_native(
        "BareBoot Workbench",
        native_options,
        Box::new(|cc| Ok(Box::new(bareboot_gui::BarebootApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    let web_options = eframe::WebOptions::default();
    wasm_bindgen_futures::spawn_local(async {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().expect("no window found");
        let document = window.document().expect("no document found");
        let canvas = document
            .get_element_by_id("bareboot_canvas")
            .expect("bareboot_canvas not found")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("element is not HtmlCanvasElement");

        let runner = eframe::WebRunner::new();
        let _ = runner
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(bareboot_gui::BarebootApp::new(cc)))),
            )
            .await;
    });
}
