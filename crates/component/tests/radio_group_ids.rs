//! RadioGroup child identity tests (upstream fix for element gallery 10.6-⑤).
//!
//! Regresses the observability defect where `RadioGroup::render` overwrote
//! every child's explicit `ElementId` with the integer group index, making
//! application-owned ids (e.g. `"al-pos-top"`) unfindable through the headless
//! observation registry and ambiguous across sibling groups in the same story.

use std::{cell::RefCell, rc::Rc};

use gpui::{Context, IntoElement, Modifiers, Render, TestAppContext, Window, div, prelude::*, px};
use gpui_base::test_support as observation;
use gpui_component::radio::{Radio, RadioGroup};

struct Harness {
    requests: Rc<RefCell<Vec<usize>>>,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let requests = self.requests.clone();
        div().w(px(400.)).child(
            RadioGroup::new("al-pos")
                .selected_index(Some(0))
                .on_click(move |index: &usize, _, _| {
                    requests.borrow_mut().push(*index);
                })
                .child(Radio::new("al-pos-top").label("Top"))
                .child(Radio::new("al-pos-right").label("Right"))
                .child(Radio::new("sc-size-large").label("Large")),
        )
    }
}

#[gpui::test]
fn radio_group_preserves_explicit_child_ids(cx: &mut TestAppContext) {
    let requests: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
    cx.update(gpui_component::init);
    let (_, visual) = cx.add_window_view({
        let requests = requests.clone();
        move |_, _| Harness { requests }
    });
    visual.update(|window, cx| window.draw(cx).clear(cx));
    visual.update(|window, _| {
        let top = observation::find(window, &[], &"al-pos-top".into())
            .expect("explicit child id \"al-pos-top\" must stay findable; RadioGroup must not overwrite it with the group index");
        assert_eq!(top.label().unwrap_or_default(), "Top");

        let large = observation::find(window, &[], &"sc-size-large".into())
            .expect("explicit child id \"sc-size-large\" must stay findable");
        assert_eq!(large.label().unwrap_or_default(), "Large");
    });
}

#[gpui::test]
fn radio_group_duplicate_ids_fall_back_to_the_group_index(cx: &mut TestAppContext) {
    struct DuplicateIds;

    impl Render for DuplicateIds {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            RadioGroup::new("dup-group")
                .child(Radio::new("same").label("First"))
                .child(Radio::new("same").label("Second"))
                .child(Radio::new("other").label("Third"))
        }
    }

    cx.update(gpui_component::init);
    let (_, visual) = cx.add_window_view(|_, _| DuplicateIds);
    visual.update(|window, cx| window.draw(cx).clear(cx));
    visual.update(|window, _| {
        // The first occurrence keeps its application-owned id.
        let first = observation::find(window, &[], &"same".into())
            .expect("the first occurrence of a duplicated id keeps the explicit id");
        assert_eq!(first.label().unwrap_or_default(), "First");
        // A later duplicate falls back to the historical index id instead of
        // drawing two elements with the same identity.
        assert!(
            observation::find(window, &[], &gpui::ElementId::Integer(1)).is_some(),
            "the duplicated occurrence falls back to the group-index id"
        );
        let other = observation::find(window, &[], &"other".into())
            .expect("non-duplicated siblings keep their ids");
        assert_eq!(other.label().unwrap_or_default(), "Third");
    });
}

#[gpui::test]
fn radio_group_activation_still_reports_the_index(cx: &mut TestAppContext) {
    let requests: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
    cx.update(gpui_component::init);
    let (_, visual) = cx.add_window_view({
        let requests = requests.clone();
        move |_, _| Harness { requests }
    });
    visual.update(|window, cx| window.draw(cx).clear(cx));

    // Click the "Right" radio by its preserved id and expect the group's
    // positional callback to still report index 1.
    let position = visual.update(|window, _| {
        let snapshot =
            observation::find(window, &[], &"al-pos-right".into()).expect("right radio registered");
        assert!(snapshot.visible());
        snapshot.bounds().center()
    });
    visual.simulate_click(position, Modifiers::default());
    assert_eq!(
        *requests.borrow(),
        vec![1],
        "clicking a named radio must still report its positional index"
    );
}
