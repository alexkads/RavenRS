//! The browser's IndexedDB as a [`Storage`]: everything is read once before the app opens,
//! reads come from memory after that, and writes go to the browser without waiting.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::Storage;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{IdbDatabase, IdbOpenDbRequest, IdbRequest, IdbTransactionMode};

/// The object store every [`IndexedDbStorage`] keeps its pieces in.
const STORE: &str = "pieces";

/// Named pieces of data kept in the browser's IndexedDB, in a database of their own.
///
/// IndexedDB answers later, while a [`Storage`] answers at once, so
/// [`IndexedDbStorage::open`] reads every piece before the app opens. Writes reach the browser
/// in the background; one that fails is kept in [`Self::problem`].
///
/// ```ignore
/// let storage = ravenrs::web::IndexedDbStorage::open("raven-contas").await?;
/// let app = App::new("Raven Contas").entity::<Account>().keep_in(storage);
/// ```
#[derive(Clone)]
pub struct IndexedDbStorage {
    database: IdbDatabase,
    pieces: Rc<RefCell<BTreeMap<String, Vec<u8>>>>,
    problem: Rc<RefCell<Option<String>>>,
}

impl fmt::Debug for IndexedDbStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IndexedDbStorage").field("database", &self.database.name()).finish_non_exhaustive()
    }
}

fn text_of(error: JsValue) -> String {
    error.as_string().or_else(|| js_sys::JSON::stringify(&error).ok().map(String::from)).unwrap_or_default()
}

/// Waits for a request to succeed or fail.
async fn finished(request: &IdbRequest) -> Result<JsValue, String> {
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let done = request.clone();
        let on_success = Closure::once_into_js(move || {
            let _ = resolve.call1(&JsValue::NULL, &done.result().unwrap_or(JsValue::UNDEFINED));
        });
        let failed = request.clone();
        let on_error = Closure::once_into_js(move || {
            let why =
                failed.error().ok().flatten().map_or_else(|| "IndexedDB failed".to_owned(), |e| e.message());
            let _ = reject.call1(&JsValue::NULL, &JsValue::from_str(&why));
        });
        request.set_onsuccess(Some(on_success.unchecked_ref()));
        request.set_onerror(Some(on_error.unchecked_ref()));
    });
    wasm_bindgen_futures::JsFuture::from(promise).await.map_err(text_of)
}

impl IndexedDbStorage {
    /// Opens, or makes, the IndexedDB database `name` and reads every piece it keeps.
    pub async fn open(name: &str) -> Result<Self, String> {
        let factory = web_sys::window()
            .ok_or("no window")?
            .indexed_db()
            .map_err(text_of)?
            .ok_or("this browser keeps no IndexedDB, as in some private windows")?;
        let request: IdbOpenDbRequest = factory.open_with_u32(name, 1).map_err(text_of)?;
        let upgrade = Closure::once_into_js({
            let request = request.clone();
            move || {
                if let Ok(result) = request.result()
                    && let Ok(database) = result.dyn_into::<IdbDatabase>()
                    && !database.object_store_names().contains(STORE)
                {
                    let _ = database.create_object_store(STORE);
                }
            }
        });
        request.set_onupgradeneeded(Some(upgrade.unchecked_ref()));
        let database: IdbDatabase = finished(&request).await?.dyn_into().map_err(text_of)?;

        let reading = database.transaction_with_str(STORE).map_err(text_of)?;
        let store = reading.object_store(STORE).map_err(text_of)?;
        let keys = finished(&store.get_all_keys().map_err(text_of)?).await?;
        let values = finished(&store.get_all().map_err(text_of)?).await?;
        let (keys, values) = (js_sys::Array::from(&keys), js_sys::Array::from(&values));
        let mut pieces = BTreeMap::new();
        for (key, value) in keys.iter().zip(values.iter()) {
            if let Some(key) = key.as_string() {
                pieces.insert(key, js_sys::Uint8Array::new(&value).to_vec());
            }
        }
        Ok(Self { database, pieces: Rc::new(RefCell::new(pieces)), problem: Rc::new(RefCell::new(None)) })
    }

    /// What went wrong the last time a write reached the browser, if it did.
    pub fn problem(&self) -> Option<String> {
        self.problem.borrow().clone()
    }

    /// Sends a change to the browser in the background.
    fn send(&self, name: &str, bytes: Option<&[u8]>) -> Result<(), String> {
        let writing = self
            .database
            .transaction_with_str_and_mode(STORE, IdbTransactionMode::Readwrite)
            .map_err(text_of)?;
        let store = writing.object_store(STORE).map_err(text_of)?;
        let key = JsValue::from_str(name);
        let request = match bytes {
            Some(bytes) => store.put_with_key(&js_sys::Uint8Array::from(bytes), &key),
            None => store.delete(&key),
        }
        .map_err(text_of)?;
        let problem = self.problem.clone();
        wasm_bindgen_futures::spawn_local(async move {
            *problem.borrow_mut() = finished(&request).await.err();
        });
        Ok(())
    }
}

impl Storage for IndexedDbStorage {
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.pieces.borrow().get(name).cloned())
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        self.pieces.borrow_mut().insert(name.to_owned(), bytes.to_vec());
        self.send(name, Some(bytes))
    }

    fn remove(&self, name: &str) -> Result<(), String> {
        self.pieces.borrow_mut().remove(name);
        self.send(name, None)
    }
}
