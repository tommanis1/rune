//! What an app adds to selectable text: its own items in the selection's menu
//! (`El::selection_menu_item`), the selection's place in the document the text was rendered
//! from (`El::source_map`), and clicks on keyed highlights (`El::on_highlight_press`).
use super::*;

#[derive(Default)]
struct State { picked: Vec<(String, Vec<std::ops::Range<usize>>)>, marks: Vec<String> }

// "aaaa bbbb" rendered from "**aaaa** bbbb": the bold markers draw nothing.
fn view(_: &State, _: &mut Cx<State>) -> El {
    col().size(300.0, 100.0)
        .selection_menu_item("comment", "Comment", |s: &mut State, sel: &crate::TextSelection, _: &mut Tx| {
            let source = sel.texts().iter().flat_map(|t| t.source().iter().cloned()).collect();
            s.picked.push((sel.text().to_owned(), source));
        })
        .on_highlight_press(|s: &mut State, key: &str, _: &mut Tx| s.marks.push(key.to_owned()))
        .child(crate::text("aaaa bbbb").key("t").font(crate::TextStyle::new(16.0, 400.0)).selectable(true)
            .source_map(vec![(2..6, 0..4), (8..13, 4..9)])
            .text_highlight(crate::TextHighlight::new(5..9, crate::oklch(0.9, 0.1, 90.0)).key("hl")))
}

fn runtime() -> Runtime<State> {
    let mut rt = Runtime::new(State::default(), view, (300.0, 100.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

#[test]
fn the_selection_menu_offers_the_apps_items_with_the_selections_source() {
    let mut rt = runtime();
    let (_, y) = rt.find("t").unwrap();
    for _ in 0..3 {
        rt.pointer_down(3.0, y);
        rt.pointer_up(3.0, y);
    }
    rt.frame(0.0);
    rt.secondary_down(3.0, y);
    rt.secondary_up(3.0, y);
    rt.frame(0.0);
    let menu = rt.popup_menu().expect("the selection menu").clone();
    assert!(menu.find("copy").is_some() && menu.find("app:comment").is_some(), "{menu:?}");
    rt.resolve_popup(Some("app:comment")).unwrap();
    rt.frame(0.0);
    assert_eq!(rt.state.picked, [("aaaa bbbb".to_owned(), vec![2..6, 8..13])], "the bold markers are left out");
}

#[test]
fn a_click_on_a_keyed_highlight_is_heard_and_a_drag_over_it_is_not() {
    let mut rt = runtime();
    let (x, y) = rt.find("hl").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
    assert_eq!(rt.state.marks, ["hl"]);
    rt.pointer_down(3.0, y);
    rt.pointer_up(3.0, y);
    rt.frame(0.0);
    assert_eq!(rt.state.marks, ["hl"], "outside the highlight");
    rt.pointer_down(x, y);
    rt.pointer_move(3.0, y);
    rt.pointer_up(3.0, y);
    rt.frame(0.0);
    assert_eq!(rt.state.marks, ["hl"], "a drag that selected text");
}

#[test]
fn selected_bytes_map_to_source_bytes() {
    use crate::runtime::selectable::to_source;
    let map = [(2..6, 0..4), (8..13, 4..9), (20..26, 9..10)];
    assert_eq!(to_source(&map, 1..3), [3..5]);
    assert_eq!(to_source(&map, 3..6), [5..6, 8..10]);
    assert_eq!(to_source(&map, 9..10), [20..26], "an entity maps as a whole");
    assert!(to_source(&map, 4..4).is_empty());
}
