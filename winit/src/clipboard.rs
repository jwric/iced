//! Access the clipboard.

use crate::core::clipboard::Kind;
use std::sync::Arc;
#[cfg(all(
    feature = "wayland",
    unix,
    not(target_vendor = "apple"),
    not(target_os = "android"),
    not(target_os = "emscripten"),
    not(target_os = "redox")
))]
use winit::platform::wayland::WindowExtWayland;
use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
use winit::window::{Window, WindowId};

/// A buffer for short-term storage and transfer within and between
/// applications.
pub struct Clipboard {
    state: State,
}

enum State {
    Connected {
        #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
        clipboard: window_clipboard::Clipboard,
        // Held until drop to satisfy the safety invariants of
        // `window_clipboard::Clipboard`.
        //
        // Note that the field ordering is load-bearing.
        #[allow(dead_code)]
        window: Arc<Window>,
        #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
        is_wayland: bool,
    },
    Unavailable,
}

impl Clipboard {
    /// Creates a new [`Clipboard`] for the given window.
    pub fn connect(window: Arc<Window>) -> Clipboard {
        let is_wayland = window.display_handle().is_ok_and(|handle| {
            matches!(handle.as_raw(), RawDisplayHandle::Wayland(_))
        });

        // SAFETY: The window handle will stay alive throughout the entire
        // lifetime of the `window_clipboard::Clipboard` because we hold
        // the `Arc<Window>` together with `State`, and enum variant fields
        // get dropped in declaration order.
        #[allow(unsafe_code)]
        let clipboard =
            unsafe { window_clipboard::Clipboard::connect(&window) };

        let state = match clipboard {
            Ok(clipboard) => State::Connected {
                clipboard,
                window,
                is_wayland,
            },
            Err(_) => State::Unavailable,
        };

        Clipboard { state }
    }

    /// Creates a new [`Clipboard`] that isn't associated with a window.
    /// This clipboard will never contain a copied value.
    pub fn unconnected() -> Clipboard {
        Clipboard {
            state: State::Unavailable,
        }
    }

    /// Reads the current content of the [`Clipboard`] as text.
    #[cfg(target_arch = "wasm32")]
    pub fn read(&self, _kind: Kind) -> Option<String> {
        // The browser has no synchronous read, and paste arrives as an input method commit instead.
        None
    }

    /// Reads the current content of the [`Clipboard`] as text.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn read(&self, kind: Kind) -> Option<String> {
        match &self.state {
            State::Connected {
                clipboard,
                window,
                is_wayland,
            } => {
                #[cfg(all(
                    feature = "wayland",
                    unix,
                    not(target_vendor = "apple"),
                    not(target_os = "android"),
                    not(target_os = "emscripten"),
                    not(target_os = "redox")
                ))]
                if kind == Kind::Standard && *is_wayland {
                    return window.selection_text();
                }

                #[cfg(not(all(
                    feature = "wayland",
                    unix,
                    not(target_vendor = "apple"),
                    not(target_os = "android"),
                    not(target_os = "emscripten"),
                    not(target_os = "redox")
                )))]
                let _ = (window, is_wayland);

                let result = match kind {
                    Kind::Standard => Some(clipboard.read()),
                    Kind::Primary => clipboard.read_primary(),
                };

                match result {
                    Some(Ok(contents)) => Some(contents),
                    Some(Err(error)) => {
                        log::warn!("error reading from clipboard: {error}");
                        None
                    }
                    None => None,
                }
            }
            State::Unavailable => None,
        }
    }

    /// Writes the given text contents to the [`Clipboard`].
    #[cfg(target_arch = "wasm32")]
    pub fn write(&mut self, _kind: Kind, contents: String) {
        write_web(&contents);
    }

    /// Writes the given text contents to the [`Clipboard`].
    #[cfg(not(target_arch = "wasm32"))]
    pub fn write(&mut self, kind: Kind, contents: String) {
        match &mut self.state {
            State::Connected {
                clipboard,
                window,
                is_wayland,
            } => {
                #[cfg(all(
                    feature = "wayland",
                    unix,
                    not(target_vendor = "apple"),
                    not(target_os = "android"),
                    not(target_os = "emscripten"),
                    not(target_os = "redox")
                ))]
                // Primary stays on window_clipboard because winit creates no primary device.
                if kind == Kind::Standard && *is_wayland {
                    window.set_selection_text(contents);
                    return;
                }

                #[cfg(not(all(
                    feature = "wayland",
                    unix,
                    not(target_vendor = "apple"),
                    not(target_os = "android"),
                    not(target_os = "emscripten"),
                    not(target_os = "redox")
                )))]
                let _ = (window, is_wayland);

                let result = match kind {
                    Kind::Standard => clipboard.write(contents),
                    Kind::Primary => {
                        clipboard.write_primary(contents).unwrap_or(Ok(()))
                    }
                };

                match result {
                    Ok(()) => {}
                    Err(error) => {
                        log::warn!("error writing to clipboard: {error}");
                    }
                }
            }
            State::Unavailable => {}
        }
    }

    /// Returns the identifier of the window used to create the [`Clipboard`], if any.
    pub fn window_id(&self) -> Option<WindowId> {
        match &self.state {
            State::Connected { window, .. } => Some(window.id()),
            State::Unavailable => None,
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn write_web(text: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };

    let clipboard = window.navigator().clipboard();
    if clipboard.is_undefined() {
        let _ = write_by_selection(&window, text);
        return;
    }

    let _ = clipboard.write_text(text);
}

#[cfg(target_arch = "wasm32")]
fn write_by_selection(window: &web_sys::Window, text: &str) -> Option<()> {
    use wasm_bindgen::JsCast;

    let document = window.document()?;
    let body = document.body()?;
    let field: web_sys::HtmlTextAreaElement =
        document.create_element("textarea").ok()?.dyn_into().ok()?;

    field.set_value(text);
    field
        .set_attribute(
            "style",
            "position:fixed;top:0;left:0;width:1px;height:1px;padding:0;border:0;opacity:0;",
        )
        .ok()?;
    field.set_attribute("readonly", "true").ok()?;
    let _ = body.append_child(&field).ok()?;

    let focused = document.active_element();
    field.select();
    let _ = document
        .unchecked_ref::<web_sys::HtmlDocument>()
        .exec_command("copy");
    field.remove();

    if let Some(focused) = focused
        .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
    {
        let _ = focused.focus();
    }

    Some(())
}

impl crate::core::Clipboard for Clipboard {
    fn read(&self, kind: Kind) -> Option<String> {
        self.read(kind)
    }

    fn write(&mut self, kind: Kind, contents: String) {
        self.write(kind, contents);
    }
}
