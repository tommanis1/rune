//! Pop-up menus Rune draws itself, where the platform has no native ones
//! ([`crate::popup_menu_supported`] false: Linux and the BSDs). The window's presenter hands the
//! menu here instead of answering `None`; it draws over everything, as the tooltip does, and
//! answers through [`Runtime::resolve_popup`], so `Tx::popup_menu` callbacks are the same as
//! with a native menu.
//!
//! It behaves as GTK's and Qt's menus do: the pointer over an enabled item highlights it, a
//! click (or a secondary release) on it picks it, a press outside dismisses the menu (that
//! press goes no further), and so do Escape, a wheel and another button. Up and Down move the
//! highlight over the enabled items, Enter or Space picks the highlighted one. While the menu
//! is open, input goes to it alone. A submenu shows inline: its title as a heading, then its
//! items indented. The menu opens with its top-left at the anchor point (its selected item
//! there, over an element) and is kept inside the window.
use super::*;
use crate::popup_menu::{Entry, PopupMenu, Show};

const FONT: f32 = 13.0;
const LINE: f32 = 16.0;
const ROW_H: f32 = 24.0;
const SEP_H: f32 = 9.0;
/// The menu's padding around its rows, and the rows' inset from its sides.
const PAD: f32 = 5.0;
/// An item's text inset within its row's highlight.
const INSET: f32 = 10.0;
/// The column of checkmarks, when an item has one.
const CHECK_W: f32 = 16.0;
/// The space between an item's title and its shortcut.
const KEY_GAP: f32 = 28.0;
/// A submenu's items' indent under its heading.
const INDENT: f32 = 12.0;
const MIN_W: f32 = 140.0;
const MARGIN: f32 = 4.0;

enum Kind {
    Item { id: String, title: String, enabled: bool, checked: bool, key: Option<String>, indent: f32 },
    Heading(String, f32),
    Separator,
}

struct Row {
    kind: Kind,
    /// Top within the menu, and height, in points.
    y: f32,
    h: f32,
}

/// The drawn menu showing: its rows laid out once at opening, where it was asked to open, and
/// where it was last placed in the window.
pub(crate) struct DrawnMenu {
    rows: Vec<Row>,
    w: f32,
    h: f32,
    /// The point the menu (or its positioned item's top-left) opens at, and how wide it must be.
    at: (f32, f32),
    shift: f32,
    check: bool,
    /// The highlighted row (an enabled item), from the pointer or the arrow keys.
    hover: Option<usize>,
    /// Top-left in window points, kept inside the window.
    origin: (f32, f32),
}

/// A shortcut as menus show it: "Ctrl+Shift+C".
fn key_label(shortcut: &crate::Shortcut) -> String {
    shortcut.to_string().split('+').map(|p| match p {
        "ctrl" => "Ctrl".to_owned(),
        "alt" => "Alt".to_owned(),
        "shift" => "Shift".to_owned(),
        "super" => "Super".to_owned(),
        k if k.chars().count() == 1 => k.to_uppercase(),
        k => { let mut c = k.chars(); c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect()) }
    }).collect::<Vec<_>>().join("+")
}

fn flatten(menu: &PopupMenu, indent: f32, out: &mut Vec<Kind>) {
    for entry in &menu.entries {
        match entry {
            Entry::Item(item) => out.push(Kind::Item {
                id: item.id.clone(), title: item.title.clone(), enabled: item.enabled, checked: item.checked,
                key: item.key_equivalent.as_ref().map(key_label), indent,
            }),
            Entry::Separator => out.push(Kind::Separator),
            Entry::Submenu(title, sub) => {
                out.push(Kind::Heading(title.clone(), indent));
                flatten(sub, indent + INDENT, out);
            }
        }
    }
}

impl DrawnMenu {
    fn rect(&self) -> [f32; 4] { [self.origin.0, self.origin.1, self.origin.0 + self.w, self.origin.1 + self.h] }

    fn contains(&self, x: f32, y: f32) -> bool {
        let r = self.rect();
        x >= r[0] && x < r[2] && y >= r[1] && y < r[3]
    }

    /// The enabled item at (x, y).
    fn item_at(&self, x: f32, y: f32) -> Option<usize> {
        if !self.contains(x, y) || x < self.origin.0 + PAD || x >= self.origin.0 + self.w - PAD { return None; }
        let ly = y - self.origin.1;
        self.rows.iter().position(|r| ly >= r.y && ly < r.y + r.h && matches!(r.kind, Kind::Item { enabled: true, .. }))
    }

    fn id(&self, row: usize) -> Option<&str> {
        match &self.rows.get(row)?.kind { Kind::Item { id, enabled: true, .. } => Some(id), _ => None }
    }

    /// The next enabled item after (`step` 1) or before (-1) the highlighted one, wrapping.
    fn step(&self, step: isize) -> Option<usize> {
        let n = self.rows.len() as isize;
        let start = self.hover.map_or(if step > 0 { -1 } else { n }, |h| h as isize);
        (1..=n).map(|k| (start + step * k).rem_euclid(n) as usize).find(|&i| self.id(i).is_some())
    }

    /// Places the menu at its anchor, kept inside a `viewport`-sized window.
    fn place(&mut self, (vw, vh): (f32, f32)) {
        let x = self.at.0.min(vw - self.w - MARGIN).max(MARGIN);
        let mut y = self.at.1 - self.shift;
        if y + self.h > vh - MARGIN {
            // No room below: above the point when it fits there, as GTK flips a context menu.
            y = if self.shift == 0.0 && self.at.1 - self.h >= MARGIN { self.at.1 - self.h } else { vh - self.h - MARGIN };
        }
        self.origin = (x, y.max(MARGIN));
    }
}

impl<S: 'static> Runtime<S> {
    /// The next menu waiting, as a drawn menu: for a window whose platform has no native menus.
    /// A request that can't open is answered `None`, as the presenter answers it. True when a
    /// menu opened or a request was answered, so the window draws.
    pub(crate) fn open_drawn_popup(&mut self) -> bool {
        let mut changed = false;
        loop {
            match self.take_popup() {
                None => return changed,
                Some(None) => { let _ = self.resolve_popup(None); changed = true; }
                Some(Some(show)) => {
                    self.drawn_popup = Some(self.lay_out_menu(show));
                    return true;
                }
            }
        }
    }

    /// Whether a drawn menu is showing.
    pub fn drawn_popup_open(&self) -> bool { self.drawn_popup.is_some() }

    /// The drawn menu's box (window points, x y w h), for tests and tools.
    pub fn drawn_popup_rect(&self) -> Option<[f32; 4]> {
        self.drawn_popup.as_ref().map(|m| [m.origin.0, m.origin.1, m.w, m.h])
    }

    /// The centre of the drawn menu's item `id` (window points), for tests and tools.
    pub fn drawn_popup_item_center(&self, id: &str) -> Option<(f32, f32)> {
        let m = self.drawn_popup.as_ref()?;
        let r = m.rows.iter().find(|r| matches!(&r.kind, Kind::Item { id: i, .. } if i == id))?;
        Some((m.origin.0 + m.w * 0.5, m.origin.1 + r.y + r.h * 0.5))
    }

    /// The highlighted item's id.
    pub fn drawn_popup_highlight(&self) -> Option<&str> {
        let m = self.drawn_popup.as_ref()?;
        m.id(m.hover?)
    }

    fn menu_text_width(&mut self, text: &str, weight: f32) -> f32 {
        let s = self.env.scale;
        let style = crate::TextStyle::new(FONT, weight).lh(LINE);
        (self.text.layout_rich(text, &[], style, s, None).full_width() / s).ceil()
    }

    fn lay_out_menu(&mut self, show: Show) -> DrawnMenu {
        let mut kinds = Vec::new();
        flatten(&show.menu, 0.0, &mut kinds);
        let check = kinds.iter().any(|k| matches!(k, Kind::Item { checked: true, .. }));
        let (mut rows, mut y, mut title_w, mut key_w, mut shift) = (Vec::new(), PAD, 0.0f32, 0.0f32, 0.0);
        for kind in kinds {
            let h = if matches!(kind, Kind::Separator) { SEP_H } else { ROW_H };
            match &kind {
                Kind::Item { id, title, key, indent, .. } => {
                    if show.positioned.as_deref() == Some(id.as_str()) { shift = y; }
                    title_w = title_w.max(indent + self.menu_text_width(title, 400.0));
                    if let Some(k) = key { key_w = key_w.max(self.menu_text_width(k, 400.0)); }
                }
                Kind::Heading(title, indent) => title_w = title_w.max(indent + self.menu_text_width(title, 600.0)),
                Kind::Separator => {}
            }
            rows.push(Row { kind, y, h });
            y += h;
        }
        let inner = if check { CHECK_W } else { 0.0 } + title_w + if key_w > 0.0 { KEY_GAP + key_w } else { 0.0 };
        let w = (2.0 * (PAD + INSET) + inner).max(MIN_W).max(show.min_width).ceil();
        let mut menu = DrawnMenu { rows, w, h: y + PAD, at: show.at, shift, check, hover: None, origin: show.at };
        menu.place(self.viewport);
        menu
    }

    /// The pick of drawn row `row`, or dismissal with `None`.
    fn answer_drawn(&mut self, row: Option<usize>) {
        let id = row.and_then(|r| self.drawn_popup.as_ref()?.id(r).map(str::to_owned));
        self.drawn_popup = None;
        if self.resolve_popup(id.as_deref()).is_err() { let _ = self.resolve_popup(None); }
    }

    /// A pointer event while a drawn menu shows: true when the menu took it (it takes every
    /// one while it's open). `down`: Some(true) a press, Some(false) a release, None a move.
    pub(super) fn drawn_popup_pointer(&mut self, x: f32, y: f32, down: Option<bool>) -> bool {
        if self.drawn_popup.is_none() { return false; }
        self.pointer = Some((x, y));
        let Some(m) = self.drawn_popup.as_mut() else { return false };
        let item = m.item_at(x, y);
        match down {
            None => m.hover = item,
            Some(true) if !m.contains(x, y) => self.answer_drawn(None),
            Some(true) => m.hover = item,
            Some(false) if item.is_some() => self.answer_drawn(item),
            Some(false) => {}
        }
        true
    }

    /// Anything else that dismisses a drawn menu (another button, a wheel): true when one showed.
    pub(super) fn drawn_popup_dismiss(&mut self) -> bool {
        if self.drawn_popup.is_none() { return false; }
        self.answer_drawn(None);
        true
    }

    /// The pointer left the window: nothing is highlighted under it.
    pub(super) fn drawn_popup_leave(&mut self) {
        if let Some(m) = self.drawn_popup.as_mut() { m.hover = None; }
    }

    /// A key while a drawn menu shows: true when the menu took it (every key, while it's open).
    pub(super) fn drawn_popup_key(&mut self, ev: &crate::KeyEvent) -> bool {
        let Some(m) = self.drawn_popup.as_mut() else { return false };
        match &ev.key {
            crate::Key::Escape => self.answer_drawn(None),
            crate::Key::Down => m.hover = m.step(1),
            crate::Key::Up => m.hover = m.step(-1),
            crate::Key::Home => { m.hover = None; m.hover = m.step(1); }
            crate::Key::End => { m.hover = None; m.hover = m.step(-1); }
            crate::Key::Enter => { let h = m.hover; if h.is_some() { self.answer_drawn(h); } }
            crate::Key::Text(t) if t == " " => { let h = m.hover; if h.is_some() { self.answer_drawn(h); } }
            _ => {}
        }
        true
    }

    /// During `build`, after the tooltip: the drawn menu over everything.
    pub(super) fn draw_drawn_popup(&mut self) {
        let Some(mut m) = self.drawn_popup.take() else { return };
        m.place(self.viewport);
        let s = self.env.scale;
        let dark = self.backdrop_dark().unwrap_or(self.env.system_dark);
        let grey = |v: f32| Color { r: v, g: v, b: v, a: 1.0 };
        let (fill, ink, faint, edge, high, shade) = if dark {
            (grey(0.16), grey(0.93), grey(0.55), Color::WHITE.alpha(0.14), Color::WHITE.alpha(0.11), Color::BLACK.alpha(0.35))
        } else {
            (grey(0.99), grey(0.11), grey(0.52), Color::BLACK.alpha(0.14), Color::BLACK.alpha(0.07), Color::BLACK.alpha(0.12))
        };
        let srgb = self.srgb_blend;
        let rect = |r: [f32; 4], radius: f32, fill: Color, border: Color| Instance {
            rect: r.map(|v| (v * s).round()),
            radii: [radius * s; 4],
            fill: premul(fill, 1.0, srgb),
            border: premul(border, 1.0, srgb),
            params: [s.max(1.0), KIND_RECT, 0.0, 1.0],
            uv: [0.0; 4],
            clip: NO_CLIP,
            fade: crate::render::pack_fade([0.0; 4], NO_CLIP, NO_CLIP),
        };
        let (ox, oy, w, h) = (m.origin.0, m.origin.1, m.w, m.h);
        // A plain offset shade under the menu, so it reads as raised over what it covers.
        self.instances.push(rect([ox, oy + 2.0, w, h], 7.0, shade, Color::TRANSPARENT));
        self.instances.push(rect([ox, oy, w, h], 7.0, fill, edge));
        let mut texts: Vec<(String, f32, f32, f32, Color)> = Vec::new();
        for (i, row) in m.rows.iter().enumerate() {
            let top = oy + row.y;
            match &row.kind {
                Kind::Separator => {
                    self.instances.push(rect([ox + PAD + INSET, top + (SEP_H - 1.0) * 0.5, w - 2.0 * (PAD + INSET), 1.0], 0.0, edge, Color::TRANSPARENT));
                }
                Kind::Heading(title, indent) => {
                    let x = ox + PAD + INSET + if m.check { CHECK_W } else { 0.0 } + indent;
                    texts.push((title.clone(), x, top, 600.0, faint));
                }
                Kind::Item { title, enabled, checked, key, indent, .. } => {
                    if m.hover == Some(i) {
                        self.instances.push(rect([ox + PAD, top, w - 2.0 * PAD, row.h], 4.0, high, Color::TRANSPARENT));
                    }
                    let color = if *enabled { ink } else { faint };
                    if *checked { texts.push(("✓".into(), ox + PAD + INSET, top, 400.0, color)); }
                    let x = ox + PAD + INSET + if m.check { CHECK_W } else { 0.0 } + indent;
                    texts.push((title.clone(), x, top, 400.0, color));
                    if let Some(k) = key { texts.push((k.clone(), f32::NAN, top, 400.0, faint)); }
                }
            }
        }
        for (text, x, top, weight, color) in texts {
            let style = crate::TextStyle::new(FONT, weight).lh(LINE);
            let layout = self.text.layout_rich(&text, &[], style, s, None);
            // NaN: a shortcut, right-aligned in the row.
            let x = if x.is_nan() { ox + w - PAD - INSET - layout.full_width() / s } else { x };
            let origin = [(x * s).round(), ((top + (ROW_H - LINE) * 0.5) * s).round()];
            self.glyph_scratch.clear();
            self.text.set_glyph_inks(color, &[]);
            self.text.glyphs_for_layout_scaled(&layout, origin, 1.0, false, &mut self.glyph_scratch);
            for q in &self.glyph_scratch {
                self.instances.extend(glyph_or_image(q, &self.text, &mut self.images, premul(color, 1.0, srgb), NO_CLIP, [0.0; 4]));
            }
        }
        self.drawn_popup = Some(m);
    }
}
