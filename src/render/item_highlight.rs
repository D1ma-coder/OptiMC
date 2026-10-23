//! Hud.extractSelectedItemName: two-second highlight, last ten ticks fade.
use crate::{inventory::Item, text::RichText};
use std::time::Instant;
#[derive(Default)]
pub(super) struct Highlight {
    last: Option<(Item, RichText)>,
    changed: Option<Instant>,
}
impl Highlight {
    pub fn update(&mut self, item: Option<&(Item, RichText)>, now: Instant) -> f32 {
        if self.last.as_ref() != item {
            self.last = item.cloned();
            self.changed = item.map(|_| now);
        }
        self.changed.map_or(0., |at| {
            ((2. - now.saturating_duration_since(at).as_secs_f32()) * 2.).clamp(0., 1.)
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn selected_name_expires_and_only_item_or_hover_name_restarts_it() {
        let now = Instant::now();
        let mut state = Highlight::default();
        let item = (Item::Registry(1), RichText::plain("Название"));
        assert_eq!(state.update(Some(&item), now), 1.);
        assert!((state.update(Some(&item), now + Duration::from_millis(1750)) - 0.5).abs() < 1e-6);
        assert_eq!(state.update(Some(&item), now + Duration::from_secs(3)), 0.);
        let renamed = (item.0, RichText::plain("Другое"));
        assert_eq!(
            state.update(Some(&renamed), now + Duration::from_secs(3)),
            1.
        );
        assert_eq!(state.update(None, now + Duration::from_secs(3)), 0.);
    }
}
