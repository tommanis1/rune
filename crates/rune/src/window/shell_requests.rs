//! What handlers and updates ask of the window itself, carried out by `Host::shell`:
//! minimizing it, showing the title they set and laying out at the zoom they set.
use super::*;

fn minimize_ignored(what: &str) {
    if cfg!(debug_assertions) { eprintln!("rune: minimize ignored: {what} can't minimize"); }
}

impl<S: 'static> Host<S> {
    /// `Tx::minimize` and `AppHandle::minimize`: miniaturize on macOS, `set_minimized`
    /// elsewhere. A hidden window, a panel or launcher, and a window that can't minimize
    /// ignore it.
    pub(super) fn minimize(&mut self) {
        #[cfg(target_os = "macos")]
        let launcher = self.launcher();
        let Some(w) = self.win.as_mut().filter(|w| w.visible) else { return minimize_ignored("a hidden window") };
        #[cfg(target_os = "macos")]
        if w.native.panel.is_some() || launcher { return minimize_ignored("a panel"); }
        // Off macOS winit reports a minimize button on a borderless window too.
        if self.options.borderless || !w.window.enabled_buttons().contains(winit::window::WindowButtons::MINIMIZE) {
            return minimize_ignored("a window without a minimize button");
        }
        #[cfg(target_os = "macos")]
        { w.native.prime = 0; }
        w.window.set_minimized(true);
    }

    /// `Tx::set_title`: shows a title set since the window last showed one, hidden or covered
    /// as it may be. A window created later opens with the current title (`try_create`).
    pub(super) fn show_title(&mut self) {
        if let Some(title) = self.rt.window_title.take_change() && let Some(w) = &self.win { w.window.set_title(title); }
    }

    /// `Tx::set_ui_zoom`: the window keeps its size on screen and lays out at the new scale,
    /// the system's times the zoom, which layout, text and hit-testing all read.
    pub(super) fn apply_zoom(&mut self) {
        if !self.rt.ui_zoom.take_change() { return; }
        let Some(w) = &self.win else { return };
        let size = w.window.inner_size();
        if size.width == 0 || size.height == 0 { return; }
        let scale = self.rt.ui_zoom.scale(w.window.scale_factor());
        self.rt.set_viewport((size.width as f32 / scale, size.height as f32 / scale), scale);
        sync_chrome(&mut self.rt, w, self.chrome, self.titlebar);
        w.window.request_redraw();
    }
}
