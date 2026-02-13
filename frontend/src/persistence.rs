//! Browser-based file persistence (save / load analysis snapshots).
//!
//! Uses the Web File API to trigger downloads and file-picker dialogs
//! directly from the WASM frontend — no server round-trip needed.

use wasm_bindgen::prelude::*;
use web_sys::{HtmlAnchorElement, Url, Blob, BlobPropertyBag, HtmlInputElement, FileReader};
use steady_invest_logic::AnalysisSnapshot;
use leptos::prelude::*;
use leptos::prelude::Callable;

/// Triggers a browser file download with the given filename and content.
///
/// Creates a temporary `<a>` element, assigns a blob URL, and clicks it.
///
/// # Errors
///
/// Returns a `JsValue` error if DOM element creation, Blob construction,
/// or URL generation fails.
pub fn trigger_download(filename: &str, content: &str) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    
    let blob_parts = js_sys::Array::new();
    blob_parts.push(&JsValue::from_str(content));
    
    let blob_options = BlobPropertyBag::new();
    blob_options.set_type("application/json");
    
    let blob = Blob::new_with_str_sequence_and_options(&blob_parts, &blob_options)?;
    let url = Url::create_object_url_with_blob(&blob)?;
    
    let a = document.create_element("a")?
        .dyn_into::<HtmlAnchorElement>()?;
    a.set_href(&url);
    a.set_download(filename);
    a.click();
    
    Url::revoke_object_url(&url)?;
    Ok(())
}

/// Serializes an analysis snapshot to JSON and triggers a browser download.
///
/// The filename includes the ticker symbol and capture timestamp.
///
/// # Errors
///
/// Returns an error if JSON serialization or the browser download trigger fails.
pub fn save_snapshot(snapshot: &AnalysisSnapshot) -> Result<(), String> {
    let json = serde_json::to_string_pretty(snapshot).map_err(|e| e.to_string())?;
    let filename = format!("steadyinvest_analysis_{}_{}.json",
        snapshot.historical_data.ticker, 
        snapshot.captured_at.format("%Y%m%d_%H%M%S")
    );
    
    trigger_download(&filename, &json).map_err(|e| format!("{:?}", e))?;
    Ok(())
}

/// Opens a file picker for `.json` / `.sinv` files and deserializes the selected file.
///
/// Calls `on_load` with the parsed [`AnalysisSnapshot`] on success,
/// or shows a browser alert if the file is corrupt.
///
/// # Errors
///
/// Returns a `JsValue` error if the DOM file input element cannot be created.
pub fn trigger_import(on_load: Callback<AnalysisSnapshot>) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    
    let input = document.create_element("input")?
        .dyn_into::<HtmlInputElement>()?;
    input.set_type("file");
    input.set_accept(".json,.sinv");
    
    let on_change = Closure::wrap(Box::new(move |ev: web_sys::Event| {
        let input: HtmlInputElement = ev.target().unwrap().dyn_into().unwrap();
        if let Some(files) = input.files() {
            if let Some(file) = files.get(0) {
                let reader = FileReader::new().unwrap();
                let on_load_inner = on_load.clone();
                let onload_callback = Closure::wrap(Box::new(move |ev: web_sys::ProgressEvent| {
                    let reader: FileReader = ev.target().unwrap().dyn_into().unwrap();
                    let content = reader.result().unwrap().as_string().unwrap();
                    if let Ok(snapshot) = serde_json::from_str::<AnalysisSnapshot>(&content) {
                        on_load_inner.run(snapshot);
                    } else {
                        let _ = web_sys::window().unwrap().alert_with_message("Corrupt or invalid analysis file. Import aborted.");
                    }
                }) as Box<dyn FnMut(web_sys::ProgressEvent)>);
                reader.set_onload(Some(onload_callback.as_ref().unchecked_ref()));
                onload_callback.forget(); // Internal to the reader, still leaks but less frequent?
                reader.read_as_text(&file).unwrap();
            }
        }
    }) as Box<dyn FnMut(web_sys::Event)>);
    
    input.set_onchange(Some(on_change.as_ref().unchecked_ref()));
    on_change.forget();
    
    input.click();
    Ok(())
}
