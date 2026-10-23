//! Subscription re-entrancy tests (upstream fix for element gallery 10.6-①).
//!
//! gpui-pre 0.3.7 invokes subscription handlers inside
//! `invoke_subscriber_in`'s `subscriber.update(...)`. A handler that calls
//! `Entity::update` on its own entity through a captured handle therefore
//! panics with "cannot update ... while it is already being updated"
//! (`entity_map.rs` double lease). These tests pin both facts: the raw pattern
//! panics, and [`gpui_base::SafeUpdateExt::update_reentrant`] resolves the
//! same situation by running inline against the handler's `&mut T`, while
//! other entities still take the regular update path.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    Context, Entity, EventEmitter, IntoElement, Render, TestAppContext, Window, div, prelude::*,
};
use gpui_base::SafeUpdateExt as _;

struct Poke;

struct Emitter {
    pings: Rc<RefCell<u32>>,
}

impl EventEmitter<Poke> for Emitter {}

/// A subscriber whose handler re-enters through its own captured handle.
struct Story {
    self_handle: Option<Entity<Self>>,
    pokes: Rc<RefCell<u32>>,
    reentrant: bool,
    // Held for the story's lifetime: dropping a Subscription unsubscribes.
    _subscription: Option<gpui::Subscription>,
}

impl Render for Story {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.self_handle.is_none() {
            self.self_handle = Some(cx.entity());
        }
        div()
    }
}

/// Builds a root view subscribing to an emitter that lives in the same app,
/// draws one frame (so the captured handle exists), then hands the emitter and
/// the visual context to `f`.
fn with_subscriber<R>(
    cx: &mut TestAppContext,
    reentrant: bool,
    pokes: Rc<RefCell<u32>>,
    f: impl FnOnce(Entity<Emitter>, &mut gpui::VisualTestContext) -> R,
) -> R {
    let emitter_handle: Rc<RefCell<Option<Entity<Emitter>>>> = Rc::new(RefCell::new(None));
    let (_, mut visual) = cx.add_window_view({
        let emitter_handle = emitter_handle.clone();
        let pokes = pokes.clone();
        move |window, cx| {
            let emitter = cx.new(|_| Emitter {
                pings: Rc::new(RefCell::new(0)),
            });
            *emitter_handle.borrow_mut() = Some(emitter.clone());
            let subscription = if reentrant {
                cx.subscribe_in(
                    &emitter,
                    window,
                    |this: &mut Story,
                     emitter: &Entity<Emitter>,
                     _: &Poke,
                     _window,
                     cx: &mut Context<Story>| {
                        let handle = this
                            .self_handle
                            .clone()
                            .expect("handle set by first render");
                        // The safe pattern: reuse the lease for the subscriber
                        // itself; other entities keep the regular update path.
                        cx.update_reentrant(this, &handle, |this, cx| {
                            *this.pokes.borrow_mut() += 1;
                            cx.notify();
                        });
                        emitter.update(cx, |emitter, _| *emitter.pings.borrow_mut() += 1);
                    },
                )
            } else {
                cx.subscribe_in(
                    &emitter,
                    window,
                    |this: &mut Story,
                     _emitter: &Entity<Emitter>,
                     _: &Poke,
                     _window,
                     cx: &mut Context<Story>| {
                        // The raw pattern from the element battle: re-enter the
                        // already-leased subscriber through its own handle.
                        let handle = this
                            .self_handle
                            .clone()
                            .expect("handle set by first render");
                        handle.update(cx, |_, cx| cx.notify());
                    },
                )
            };
            Story {
                self_handle: None,
                pokes,
                reentrant,
                _subscription: Some(subscription),
            }
        }
    });
    visual.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    let emitter = emitter_handle.borrow().clone().expect("emitter exists");
    f(emitter, &mut visual)
}

#[gpui::test]
#[should_panic(expected = "while it is already being updated")]
fn raw_self_update_inside_a_subscription_panics(cx: &mut TestAppContext) {
    with_subscriber(cx, false, shared_pokes(), |emitter, visual| {
        // Emitting ends with a effects flush, which delivers the subscription
        // while the subscriber is leased; the handler then double-leases it.
        visual.update(|_, cx| emitter.update(cx, |_, cx| cx.emit(Poke)));
    });
}

#[gpui::test]
fn reentrant_update_resolves_the_double_lease(cx: &mut TestAppContext) {
    let pokes = Rc::new(RefCell::new(0));
    with_subscriber(cx, true, pokes.clone(), |emitter, visual| {
        visual.update(|_, cx| emitter.update(cx, |_, cx| cx.emit(Poke)));
    });
    assert_eq!(
        *pokes.borrow(),
        1,
        "the handler must run exactly once despite re-entering its own entity"
    );
}

fn shared_pokes() -> Rc<RefCell<u32>> {
    Rc::new(RefCell::new(0))
}
