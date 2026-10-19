use anyhow::anyhow;
use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use wasm_bindgen_futures::spawn_local;

/// A redraw callback invoked after an on-demand asset finishes downloading.
///
/// The callback runs on the WASM main thread after the download completes.
/// Because `AssetSource::load` is synchronous and returns an error while the
/// asset is still in flight, GPUI has no built-in way to know when the bytes
/// become available; without this hook the host application would never repaint
/// and freshly downloaded SVG/PNG icons would stay invisible until the next
/// unrelated redraw. The callback must be `Send + Sync` (a zero-capture closure
/// referencing a `thread_local!` handle satisfies this on the single-threaded
/// WASM target) and should schedule a redraw, e.g. by refreshing every window.
type OnDownloaded = Arc<RwLock<Option<Box<dyn Fn() + Send + Sync>>>>;

/// WASM implementation - download assets on-demand
pub struct Assets {
    endpoint: SharedString,
    cache: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    pending: Arc<RwLock<HashMap<String, bool>>>,
    on_downloaded: OnDownloaded,
}

impl Assets {
    pub fn new(endpoint: impl Into<SharedString>) -> Self {
        Self {
            endpoint: endpoint.into(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            pending: Arc::new(RwLock::new(HashMap::new())),
            on_downloaded: Arc::new(RwLock::new(None)),
        }
    }

    /// Register a callback invoked after each on-demand asset download
    /// completes, so the host application can schedule a redraw and display
    /// the freshly cached bytes.
    ///
    /// Call this before passing `Assets` to `Application::with_assets`; the
    /// callback is shared through an `Arc`, so it stays visible after the
    /// `Assets` value is moved into the application.
    ///
    /// ## Usage
    ///
    /// ```ignore
    /// // A zero-capture closure that reaches the app through a `thread_local!`
    /// // handle is `Send + Sync` and therefore accepted here.
    /// assets.set_on_downloaded(|| {
    ///     APP_HANDLE.with(|slot| {
    ///         if let Some(handle) = slot.borrow().as_ref() {
    ///             handle.update(|cx| {
    ///                 for window in cx.windows() {
    ///                     let _ = window.update(cx, |_, window, _| window.refresh());
    ///                 }
    ///             });
    ///         }
    ///     });
    /// });
    /// ```
    pub fn set_on_downloaded(&self, callback: impl Fn() + Send + Sync + 'static) {
        if let Ok(mut slot) = self.on_downloaded.write() {
            *slot = Some(Box::new(callback));
        }
    }
}

impl Default for Assets {
    fn default() -> Self {
        Self::new("")
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        if path.starts_with("icons/") && (path.ends_with(".svg") || path.ends_with(".png")) {
            // Check if already cached
            if let Ok(cache) = self.cache.read() {
                if let Some(data) = cache.get(path) {
                    return Ok(Some(Cow::Owned(data.clone())));
                }
            }

            // Check if download is already pending
            let is_pending = self
                .pending
                .read()
                .map(|p| p.contains_key(path))
                .unwrap_or(false);

            if !is_pending {
                // Mark as pending and start download
                if let Ok(mut pending) = self.pending.write() {
                    pending.insert(path.to_string(), true);
                }

                let url = format!("{}/assets/{}", self.endpoint, path);
                let path_clone = path.to_string();
                let cache = self.cache.clone();
                let pending = self.pending.clone();
                let on_downloaded = self.on_downloaded.clone();

                spawn_local(async move {
                    let mut downloaded = false;
                    match reqwest::get(&url).await {
                        Ok(response) => {
                            if response.status().is_success() {
                                match response.bytes().await {
                                    Ok(bytes) => {
                                        if let Ok(mut cache) = cache.write() {
                                            cache.insert(path_clone.clone(), bytes.to_vec());
                                            downloaded = true;
                                        }
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to read icon {}: {}", path_clone, e);
                                    }
                                }
                            } else {
                                log::warn!(
                                    "Failed to download icon {}: HTTP {}",
                                    path_clone,
                                    response.status()
                                );
                            }
                        }
                        Err(e) => {
                            log::warn!("Failed to fetch icon {}: {}", path_clone, e);
                        }
                    }

                    // Remove from pending
                    if let Ok(mut pending) = pending.write() {
                        pending.remove(&path_clone);
                    }

                    // Now that the bytes are cached, ask the host app to
                    // repaint so the freshly downloaded asset becomes visible
                    // without waiting for an unrelated user interaction.
                    //
                    // The callback only refreshes windows (it never re-enters
                    // `set_on_downloaded`), so invoking it while holding the
                    // read guard cannot deadlock.
                    if downloaded
                        && let Ok(slot) = on_downloaded.read()
                        && let Some(callback) = slot.as_ref()
                    {
                        callback();
                    }
                });
            }

            // Return error so GPUI will retry (but only log once per icon)
            Err(anyhow!("Wasm assets loading, will be available soon..."))
        } else {
            Ok(None)
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let _ = path;
        Ok(Vec::new())
    }
}
