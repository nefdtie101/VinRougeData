use js_sys::{ArrayBuffer, Function, Promise, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{File, FileReader};

/// Reads a browser `File` object into a `Vec<u8>` asynchronously.
pub async fn read_file_as_bytes(file: &File) -> Result<Vec<u8>, JsValue> {
    let reader = FileReader::new()?;

    let promise = Promise::new(&mut |resolve, reject| {
        // Clone inside the FnMut body so we don't have to move out of it.
        let reader_snap = reader.clone();
        let reject_snap = reject.clone();

        // once_into_js takes a true FnOnce — the cloned values can be moved freely.
        let on_load: JsValue =
            wasm_bindgen::closure::Closure::once_into_js(move || match reader_snap.result() {
                Ok(val) => match val.dyn_into::<ArrayBuffer>() {
                    Ok(buf) => {
                        let _ = resolve.call1(&JsValue::NULL, &Uint8Array::new(&buf));
                    }
                    Err(_) => {
                        let _ =
                            reject.call1(&JsValue::NULL, &JsValue::from_str("Not an ArrayBuffer"));
                    }
                },
                Err(e) => {
                    let _ = reject.call1(&JsValue::NULL, &e);
                }
            });

        let on_error: JsValue = wasm_bindgen::closure::Closure::once_into_js(move |e: JsValue| {
            let _ = reject_snap.call1(&JsValue::NULL, &e);
        });

        reader.set_onload(Some(on_load.unchecked_ref::<Function>()));
        reader.set_onerror(Some(on_error.unchecked_ref::<Function>()));
        reader.read_as_array_buffer(file).unwrap_or(());
    });

    let result = JsFuture::from(promise).await?;
    let typed = result.dyn_into::<Uint8Array>()?;
    Ok(typed.to_vec())
}
