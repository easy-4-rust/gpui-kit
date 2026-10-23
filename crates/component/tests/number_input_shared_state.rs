//! Shared-state NumberInput multi-instance identity tests (upstream fix for
//! element gallery 10.6-③).
//!
//! The official input-number demos render several spinbuttons sharing one
//! `InputState` (one v-model). Under unnamed ancestors all instances paint the
//! same GPUI element identity `[("number-input", state_id), "increment"]`, and
//! gpui-pre merges their element state: the observation registry keeps only
//! the last-painted pane and click activation becomes undefined. The fix is
//! the explicit, stable [`NumberInput::instance_id`] namespace; collapsing
//! without one remains the documented upstream limit (the element gallery's
//! shared-pane acceptance drives those panes by keyboard, which is unaffected).

use gpui::{
    Context, Entity, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use gpui_base::test_support as observation;
use gpui_component::input::{InputState, NumberInput};

const PANE_WIDTH: f32 = 160.;
const PANE_GAP: f32 = 20.;

struct SharedPanes {
    state: Entity<InputState>,
    first_id: Option<&'static str>,
    second_id: Option<&'static str>,
}

impl Render for SharedPanes {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut first = NumberInput::new(&self.state);
        let mut second = NumberInput::new(&self.state);
        if let Some(id) = self.first_id {
            first = first.instance_id(id);
        }
        if let Some(id) = self.second_id {
            second = second.instance_id(id);
        }
        // Two *unnamed* wrappers: the layout that merges unnamed instances.
        div().flex().gap(px(PANE_GAP)).child(
            div()
                .w(px(PANE_WIDTH))
                .child(first)
                .child(div().w(px(PANE_WIDTH)).child(second)),
        )
    }
}

fn draw(visual: &mut gpui::VisualTestContext) {
    visual.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

/// Centers of every "increment" button, ordered by x (document order).
fn increment_centers(visual: &mut gpui::VisualTestContext) -> Vec<gpui::Point<Pixels>> {
    let mut centers: Vec<gpui::Point<Pixels>> = visual.update(|window, _| {
        observation::snapshots(window)
            .iter()
            .filter(|snapshot| snapshot.path().last() == Some(&"increment".into()))
            .map(|snapshot| snapshot.bounds().center())
            .collect()
    });
    centers.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
    centers
}

/// Pointer rhythm with a framework redraw between down and up, as harnesses
/// render frames between events.
fn activate_with_redraw(visual: &mut gpui::VisualTestContext, position: gpui::Point<Pixels>) {
    use gpui::{InputEvent as _, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent};
    visual.update(|window, cx| {
        window.dispatch_event(
            MouseMoveEvent {
                position,
                pressed_button: None,
                modifiers: Default::default(),
            }
            .to_platform_input(),
            cx,
        );
        window.refresh();
        window.draw(cx).clear(cx);
        window.dispatch_event(
            MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }
            .to_platform_input(),
            cx,
        );
        window.refresh();
        window.draw(cx).clear(cx);
        window.dispatch_event(
            MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers: Default::default(),
                click_count: 1,
            }
            .to_platform_input(),
            cx,
        );
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

#[gpui::test]
fn instance_ids_give_each_shared_instance_its_own_identity(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (view, mut visual) = cx.add_window_view(|window, cx| SharedPanes {
        state: cx.new(|cx| {
            gpui_component::input::InputState::new(window, cx)
                .default_value("1")
                .step(1.)
        }),
        first_id: Some("pane-a"),
        second_id: Some("pane-b"),
    });
    draw(&mut visual);
    let state = visual.update(|_, cx| view.read(cx).state.clone());

    // Each pane paints its own increment identity inside its own namespace.
    let paths: Vec<String> = visual.update(|window, _| {
        let mut paths: Vec<String> = observation::snapshots(window)
            .iter()
            .filter(|snapshot| snapshot.path().last() == Some(&"increment".into()))
            .map(|snapshot| format!("{:?}", snapshot.path()))
            .collect();
        paths.sort();
        paths
    });
    assert_eq!(
        paths.len(),
        2,
        "each named instance must paint its own increment identity, got {paths:?}"
    );

    let centers = increment_centers(&mut visual);
    assert_eq!(centers.len(), 2);

    // With stable identities both panes activate across harness redraws.
    activate_with_redraw(&mut visual, centers[0]);
    let after_first = visual.update(|_, cx| state.read(cx).value().to_string());
    assert_eq!(
        after_first, "2",
        "the first named instance steps the shared value"
    );

    activate_with_redraw(&mut visual, centers[1]);
    let after_second = visual.update(|_, cx| state.read(cx).value().to_string());
    assert_eq!(
        after_second, "3",
        "the second named instance steps the shared value too"
    );
}

#[gpui::test]
fn unnamed_shared_instances_stay_collapsed(cx: &mut TestAppContext) {
    // Documents the upstream limit: without explicit instance ids, all panes
    // bound to one state paint one identity and the registry keeps only the
    // last-painted pane (keyboard activation still works, which is how the
    // element gallery acceptance drives shared panes).
    cx.update(gpui_component::init);
    let (_, mut visual) = cx.add_window_view(|window, cx| SharedPanes {
        state: cx.new(|cx| {
            gpui_component::input::InputState::new(window, cx)
                .default_value("1")
                .step(1.)
        }),
        first_id: None,
        second_id: None,
    });
    draw(&mut visual);

    let paths: Vec<String> = visual.update(|window, _| {
        observation::snapshots(window)
            .iter()
            .filter(|snapshot| snapshot.path().last() == Some(&"increment".into()))
            .map(|snapshot| format!("{:?}", snapshot.path()))
            .collect()
    });
    assert_eq!(
        paths.len(),
        1,
        "unnamed instances collapse into the last-painted identity: {paths:?}"
    );
}

#[gpui::test]
fn separate_state_instances_step_with_redraws_by_default(cx: &mut TestAppContext) {
    // Regression guard: panes with their own states never merged; the default
    // (no instance ids) behavior must stay unchanged.
    struct SeparatePanes {
        state_a: Entity<InputState>,
        state_b: Entity<InputState>,
    }

    impl Render for SeparatePanes {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .gap(px(PANE_GAP))
                .child(
                    div()
                        .w(px(PANE_WIDTH))
                        .child(NumberInput::new(&self.state_a)),
                )
                .child(
                    div()
                        .w(px(PANE_WIDTH))
                        .child(NumberInput::new(&self.state_b)),
                )
        }
    }

    cx.update(gpui_component::init);
    let (view, mut visual) = cx.add_window_view(|window, cx| SeparatePanes {
        state_a: cx.new(|cx| {
            gpui_component::input::InputState::new(window, cx)
                .default_value("1")
                .step(1.)
        }),
        state_b: cx.new(|cx| {
            gpui_component::input::InputState::new(window, cx)
                .default_value("1")
                .step(1.)
        }),
    });
    draw(&mut visual);

    let centers = increment_centers(&mut visual);
    assert_eq!(centers.len(), 2);

    for (center, name) in centers.iter().zip(["state_a", "state_b"]) {
        activate_with_redraw(&mut visual, *center);
        let value = visual.update(|_, cx| match name {
            "state_a" => view.read(cx).state_a.read(cx).value().to_string(),
            _ => view.read(cx).state_b.read(cx).value().to_string(),
        });
        assert_eq!(
            value, "2",
            "{name}'s increment must step with redraws in between"
        );
    }
}
