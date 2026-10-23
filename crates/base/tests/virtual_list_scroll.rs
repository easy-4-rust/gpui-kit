//! VirtualList scroll persistence tests (upstream fix for element gallery
//! 10.6-④).
//!
//! Regresses the defect where `v_virtual_list`/`h_virtual_list` built a fresh
//! `VirtualListScrollHandle` on every construction. Owners that rebuild the
//! list each render (virtual tables, infinite scroll) thereby reset the scroll
//! offset to zero every frame, so wheel scrolling never sticks.

use std::{cell::RefCell, rc::Rc};

use gpui::{
    Context, InputEvent as _, IntoElement, Pixels, Render, ScrollDelta, ScrollWheelEvent, Size,
    TestAppContext, Window, div, point, prelude::*, px, size,
};
use gpui_base::{
    TestSupportExt as _, VirtualListScrollHandle, test_support as observation, v_virtual_list,
};

fn render_items(range: std::ops::Range<usize>) -> Vec<impl IntoElement> {
    range
        .map(|ix| {
            div()
                .id(("vl-item", ix))
                .test_support()
                .w(px(300.))
                .h(px(24.))
                .child(format!("item {ix}"))
        })
        .collect()
}

fn item_sizes() -> Rc<Vec<Size<Pixels>>> {
    Rc::new((0..100).map(|_| size(px(300.), px(24.))).collect())
}

fn draw(visual: &mut gpui::VisualTestContext) {
    visual.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

fn scroll_down_at(visual: &mut gpui::VisualTestContext, position: gpui::Point<Pixels>, dy: f32) {
    visual.update(|window, cx| {
        window.dispatch_event(
            ScrollWheelEvent {
                position,
                delta: ScrollDelta::Pixels(point(px(0.), px(dy))),
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

fn first_item_center(visual: &mut gpui::VisualTestContext) -> gpui::Point<Pixels> {
    visual
        .update(|window, _| {
            observation::find(window, &[], &("vl-item", 0usize).into())
                .map(|item| item.bounds().center())
        })
        .expect("first item must be registered on the drawn frame")
}

/// The real-world pattern under test: the owner rebuilds the scroll handle on
/// every render and keeps the latest one so tests can read the live offset.
struct ListHost {
    last_handle: Rc<RefCell<Option<VirtualListScrollHandle>>>,
}

impl Render for ListHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        // A fresh handle every frame: the exact usage that loses the position.
        let handle = VirtualListScrollHandle::new();
        *self.last_handle.borrow_mut() = Some(handle.clone());

        v_virtual_list(view, "big-list", item_sizes(), |_, range, _, _| {
            render_items(range)
        })
        .track_scroll(&handle)
    }
}

#[gpui::test]
fn rebuilt_scroll_handle_keeps_the_offset_across_re_renders(cx: &mut TestAppContext) {
    let last_handle: Rc<RefCell<Option<VirtualListScrollHandle>>> = Rc::new(RefCell::new(None));
    let (view, mut visual) = cx.add_window_view({
        let last_handle = last_handle.clone();
        move |_, _| ListHost {
            last_handle: last_handle.clone(),
        }
    });
    draw(&mut visual);

    // Scroll the list by 200 px through a real wheel event on the first item.
    let item_center = first_item_center(&mut visual);
    scroll_down_at(&mut visual, item_center, -200.);

    let scrolled = last_handle
        .borrow()
        .as_ref()
        .expect("handle captured")
        .offset();
    assert!(
        scrolled.y < px(0.),
        "wheel scrolling must move the list, got offset {scrolled:?}"
    );

    // Re-render (a fresh handle is built this frame) — the offset must survive.
    visual.update(|_, cx| view.update(cx, |_, cx| cx.notify()));
    draw(&mut visual);

    let after = last_handle
        .borrow()
        .as_ref()
        .expect("handle captured")
        .offset();
    assert_eq!(
        after, scrolled,
        "a re-render must not reset the scroll offset of a rebuilt handle"
    );
}

#[gpui::test]
fn persistent_scroll_handle_keeps_the_offset_across_re_renders(cx: &mut TestAppContext) {
    // Regression guard: an owner that keeps one handle (tables) never resets.
    struct PersistentHost {
        handle: VirtualListScrollHandle,
    }

    impl Render for PersistentHost {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let view = cx.entity();
            let handle = self.handle.clone();
            v_virtual_list(view, "big-list", item_sizes(), |_, range, _, _| {
                render_items(range)
            })
            .track_scroll(&handle)
        }
    }

    let (view, mut visual) = cx.add_window_view(|_, _| PersistentHost {
        handle: VirtualListScrollHandle::new(),
    });
    draw(&mut visual);

    let item_center = first_item_center(&mut visual);
    scroll_down_at(&mut visual, item_center, -200.);

    // Re-render with the same persistent handle.
    visual.update(|_, cx| view.update(cx, |_, cx| cx.notify()));
    draw(&mut visual);

    visual.update(|_, cx| {
        let offset = view.read(cx).handle.offset();
        assert!(
            offset.y < px(0.),
            "a persistent handle keeps its offset across re-renders, got {offset:?}"
        );
    });
}
