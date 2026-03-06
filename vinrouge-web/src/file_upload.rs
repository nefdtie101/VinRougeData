use js_sys::{ArrayBuffer, Promise, Uint8Array};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{File, FileReader};

/// Reads a browser `File` object into a `Vec<u8>` asynchronously.
///
/// Uses the FileReader API via a JS Promise bridge so it can be
/// awaited from Rust async code running under wasm-bindgen-futures.
pub async fn read_file_as_bytes(file: &File) -> Result<Vec<u8>, JsValue> {
    let reader = FileReader::new().map_err(|e| e)?;
    let reader_clone = reader.clone();

    let promise = Promise::new(&mut |resolve, reject| {
        let on_load: Closure<dyn FnMut()> = Closure::once(Box::new(move || {
            let result = match reader_clone.result() {
                Ok(v) => v,
                Err(e) => {
                    let _ = reject.call1(&JsValue::NULL, &e);
                    return;
                }
            };
            let array_buffer = match result.dyn_into::<ArrayBuffer>() {
                Ok(buf) => buf,
                Err(_) => {
                    let _ = reject.call1(&JsValue::NULL, &JsValue::from_str("Not an ArrayBuffer"));
                    return;
                }
            };
            let typed = Uint8Array::new(&array_buffer);
            let _ = resolve.call1(&JsValue::NULL, &typed);
        }));

        let on_error: Closure<dyn FnMut(JsValue)> = Closure::once(Box::new(move |e: JsValue| {
            let _ = reject.call1(&JsValue::NULL, &e);
        }));

        reader.set_onload(Some(on_load.as_ref().unchecked_ref()));
        reader.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        reader.read_as_array_buffer(file).unwrap_or(());

        on_load.forget();
        on_error.forget();
    });

    let result = JsFuture::from(promise).await?;
    let typed = result.dyn_into::<Uint8Array>()?;
    Ok(typed.to_vec())
}
