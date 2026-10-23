//! Re-entrancy-safe entity updates for subscription and observer handlers.
//!
//! gpui-pre 0.3.7 leases the subscriber before invoking a subscription or
//! observer callback (`invoke_subscriber_in` / `invoke_observer_in` run the
//! handler inside `subscriber.update(...)`). When the handler calls
//! `Entity::update` on the *same* entity through a captured handle, the second
//! lease panics in `entity_map.rs` with "cannot update ... while it is already
//! being updated" (element gallery 10.6-①, gpui-pre `entity_map.rs:164`).
//!
//! [`SafeUpdateExt::update_reentrant`] closes that trap without touching
//! gpui-pre: when the target entity is the context's own (already-leased)
//! entity, the closure runs inline against the handler's own `&mut T`
//! parameter instead of leasing again; any other entity takes the regular
//! `Entity::update` path.

use gpui::{Context, Entity};

/// Lease-safe updates from inside subscription/observer handlers.
pub trait SafeUpdateExt<T: 'static> {
    /// Updates `entity`, reusing the context's existing lease when `entity`
    /// is the entity this context belongs to.
    ///
    /// In a `subscribe_in`/`subscribe`/`observe` handler, pass the handler's
    /// own `&mut T` as `this`. When `entity` is the subscriber itself (the
    /// double-lease case that panics on `Entity::update`), `f` runs inline
    /// with that `this`; otherwise `f` runs through a normal
    /// `Entity::update`. Either way `f` receives `&mut T` and the entity's
    /// `Context<T>`, so `cx.notify()` behaves identically on both paths.
    fn update_reentrant<R>(
        &mut self,
        this: &mut T,
        entity: &Entity<T>,
        f: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R;
}

impl<T: 'static> SafeUpdateExt<T> for Context<'_, T> {
    fn update_reentrant<R>(
        &mut self,
        this: &mut T,
        entity: &Entity<T>,
        f: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R {
        if entity.entity_id() == self.entity_id() {
            // The entity is already leased by the callback invocation; the
            // handler's `this` *is* that lease. Run inline, never re-lease.
            f(this, self)
        } else {
            entity.update(self, f)
        }
    }
}
