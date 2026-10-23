//! InputState::set_placeholder change-guard tests (upstream fix for element
//! gallery 10.6-②).
//!
//! Render-phase code syncs props into the shared `InputState` through
//! `window.defer` + `set_placeholder`. The setter used to assign and notify
//! unconditionally, so every frame re-wrote the same placeholder and kept the
//! window dirty forever: "render → defer → notify → dirty → redraw" never
//! converges under gpui's deterministic test scheduling (the exact hang the
//! element gallery hit with its ElInput placeholder sync). The setter must
//! skip the write and the notify when the text is unchanged.

use std::{cell::Cell as StdCell, cell::RefCell, rc::Rc};

use gpui::{Context, Entity, IntoElement, Render, TestAppContext, Window, div, prelude::*};
use gpui_base::input::InputState;

struct PlaceholderHost {
    state: Entity<InputState>,
}

impl Render for PlaceholderHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(gpui_base::input::Input::new(&self.state))
    }
}

#[gpui::test]
fn set_placeholder_skips_notify_when_unchanged(cx: &mut TestAppContext) {
    let notifies = Rc::new(StdCell::new(0u32));
    let state_cell: Rc<RefCell<Option<Entity<InputState>>>> = Rc::new(RefCell::new(None));

    let notifies_for_view = notifies.clone();
    let state_for_view = state_cell.clone();
    let (_, mut visual) = cx.add_window_view(move |window, cx| {
        let state = cx.new(|cx| gpui_base::input::InputState::new(window, cx));
        let notifies_for_observer = notifies_for_view.clone();
        cx.observe(&state, move |_, _, _| {
            notifies_for_observer.set(notifies_for_observer.get() + 1);
        })
        .detach();
        *state_for_view.borrow_mut() = Some(state.clone());
        PlaceholderHost { state }
    });
    visual.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    let state = state_cell.borrow().clone().expect("state exists");

    // The first write changes the placeholder and must notify.
    let before = notifies.get();
    visual.update(|window, cx| {
        state.update(cx, |state, cx| state.set_placeholder("hint", window, cx));
    });
    assert_eq!(
        notifies.get(),
        before + 1,
        "a real placeholder change notifies observers"
    );

    // Re-writing the SAME text (what every render pass does) must not notify,
    // otherwise deterministic test scheduling never converges.
    let after_change = notifies.get();
    for _ in 0..3 {
        visual.update(|window, cx| {
            state.update(cx, |state, cx| state.set_placeholder("hint", window, cx));
        });
    }
    assert_eq!(
        notifies.get(),
        after_change,
        "an unchanged placeholder write must be a no-op (no notify, no redraw loop)"
    );

    // A real change afterwards still notifies.
    visual.update(|window, cx| {
        state.update(cx, |state, cx| state.set_placeholder("other", window, cx));
    });
    assert_eq!(
        notifies.get(),
        after_change + 1,
        "a later real change keeps notifying"
    );
}
