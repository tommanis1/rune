//! The menu Rune draws where the platform has no native one (`drawn_popup.rs`): a selection's
//! context menu opens, is picked by a click or the keys, and goes away on Escape or a press
//! outside it, answering through the same callback as a native menu.
use super::*;

#[derive(Default)]
struct State { picked: Vec<String> }

fn view(_: &State, _: &mut Cx<State>) -> El {
    col().size(400.0, 300.0)
        .selection_menu_item("comment", "Comment", |s: &mut State, sel: &crate::TextSelection, _: &mut Tx| {
            s.picked.push(sel.text().to_owned());
        })
        .child(crate::text("aaaa bbbb").key("t").font(crate::TextStyle::new(16.0, 400.0)).selectable(true))
}

/// The word under (3, y) selected and its menu drawn, as a right-click leaves it.
fn opened() -> Runtime<State> {
    let mut rt = Runtime::new(State::default(), view, (400.0, 300.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let (_, y) = rt.find("t").unwrap();
    rt.secondary_down(3.0, y);
    rt.secondary_up(3.0, y);
    rt.frame(0.0);
    assert!(rt.open_drawn_popup(), "the selection's menu opens drawn");
    rt.frame(0.0);
    assert!(rt.drawn_popup_open());
    rt
}

#[test]
fn a_click_on_an_item_picks_it_through_the_menus_callback() {
    let mut rt = opened();
    let [x, y, w, h] = rt.drawn_popup_rect().unwrap();
    assert!(x >= 0.0 && y >= 0.0 && x + w <= 400.0 && y + h <= 300.0, "inside the window");
    let (cx, cy) = rt.drawn_popup_item_center("app:comment").expect("the app's item is drawn");
    rt.pointer_move(cx, cy);
    assert_eq!(rt.drawn_popup_highlight(), Some("app:comment"), "the pointer highlights it");
    rt.pointer_down(cx, cy);
    rt.pointer_up(cx, cy);
    rt.frame(0.0);
    assert!(!rt.drawn_popup_open() && rt.popup_menu().is_none());
    assert_eq!(rt.state.picked, ["aaaa"]);
}

#[test]
fn the_keys_move_the_highlight_and_enter_picks() {
    let mut rt = opened();
    let key = |key| crate::KeyEvent { key, mods: Default::default() };
    rt.key(key(crate::Key::Down));
    assert_eq!(rt.drawn_popup_highlight(), Some("copy"));
    rt.key(key(crate::Key::Down));
    assert_eq!(rt.drawn_popup_highlight(), Some("app:comment"));
    rt.key(key(crate::Key::Down));
    assert_eq!(rt.drawn_popup_highlight(), Some("copy"), "it wraps");
    rt.key(key(crate::Key::Up));
    rt.key(key(crate::Key::Enter));
    rt.frame(0.0);
    assert!(!rt.drawn_popup_open());
    assert_eq!(rt.state.picked, ["aaaa"]);
}

#[test]
fn escape_and_a_press_outside_dismiss_it_and_the_press_goes_no_further() {
    let mut rt = opened();
    rt.key(crate::KeyEvent { key: crate::Key::Escape, mods: Default::default() });
    rt.frame(0.0);
    assert!(!rt.drawn_popup_open() && rt.popup_menu().is_none() && rt.state.picked.is_empty());

    let mut rt = opened();
    let [x, y, w, h] = rt.drawn_popup_rect().unwrap();
    let outside = if x + w + 20.0 < 400.0 { (x + w + 10.0, y + h * 0.5) } else { (x - 10.0, y + h * 0.5) };
    rt.pointer_down(outside.0, outside.1);
    rt.pointer_up(outside.0, outside.1);
    rt.frame(0.0);
    assert!(!rt.drawn_popup_open() && rt.popup_menu().is_none() && rt.state.picked.is_empty());
}

#[test]
fn a_menu_near_the_edge_stays_inside_the_window() {
    let mut rt = Runtime::new(State::default(), view, (400.0, 300.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let (_, y) = rt.find("t").unwrap();
    rt.secondary_down(3.0, y);
    rt.secondary_up(3.0, y);
    rt.frame(0.0);
    // Moved to the bottom-right corner, as a right-click there would ask.
    rt.popups.front_mut().unwrap().anchor = crate::PopupAnchor::Point(395.0, 295.0);
    assert!(rt.open_drawn_popup());
    rt.frame(0.0);
    let [x, y, w, h] = rt.drawn_popup_rect().unwrap();
    assert!(x + w <= 400.0 && y + h <= 300.0 && x >= 0.0 && y >= 0.0, "{:?}", [x, y, w, h]);
}
