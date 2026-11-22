//! Logical design units. At the default text scale, one rem is 16 logical pixels.
use crate::Theme;
use gpui::{App, Rems, Window, px, rems};
use std::time::Duration;

pub mod text {
    use super::*;
    pub const CAPTION: Rems = rems(0.75);
    pub const CONTROL: Rems = rems(0.875);
    pub const BODY: Rems = rems(1.0);
    pub const TITLE: Rems = rems(1.25);
}
pub mod space {
    use super::*;
    pub const XS: Rems = rems(0.25);
    pub const SM: Rems = rems(0.5);
    pub const MD: Rems = rems(0.75);
    pub const LG: Rems = rems(1.);
    pub const XL: Rems = rems(1.5);
}
pub mod radius {
    use super::*;
    pub const CONTROL: Rems = rems(0.375);
    pub const DIALOG: Rems = rems(0.5);
}
pub mod icon {
    use super::*;
    pub const SMALL: Rems = rems(0.875);
    pub const NORMAL: Rems = rems(1.);
    pub const LARGE: Rems = rems(1.5);
    pub const EXTRA_LARGE: Rems = rems(2.);
}
#[derive(Clone, Copy)]
pub enum Motion {
    Feedback,
    Transition,
}
impl Motion {
    pub fn duration(self, reduced: bool) -> Duration {
        Duration::from_millis(if reduced {
            0
        } else {
            match self {
                Self::Feedback => 100,
                Self::Transition => 180,
            }
        })
    }
    pub fn current(self, cx: &App) -> Duration {
        self.duration(cx.reduce_motion())
    }
}
impl Theme {
    pub fn muted(self) -> u32 {
        self.secondary
    }
    pub fn selected(self) -> u32 {
        self.selection
    }
    pub fn disabled(self) -> u32 {
        if self.high_contrast {
            self.text
        } else if self.light {
            0x58635c
        } else {
            0x99a59c
        }
    }
    pub fn error(self) -> u32 {
        if self.light { 0x952e2b } else { 0xffb3a9 }
    }
}
pub fn set_text_scale(window: &mut Window, factor: f32) {
    let factor = if factor.is_finite() && factor > 0. {
        factor
    } else {
        1.
    };
    window.set_rem_size(px(16. * factor));
}
#[cfg(test)]
mod tests {
    use super::{Motion, Theme};
    #[test]
    fn reduced_motion_and_state_colors() {
        assert!(Motion::Transition.duration(true).is_zero());
        assert!(Motion::Feedback.duration(false) < Motion::Transition.duration(false));
        for theme in [Theme::LIGHT, Theme::DARK, Theme::HIGH_CONTRAST] {
            assert_ne!(theme.disabled(), theme.surface);
            assert_ne!(theme.error(), theme.panel);
            assert_eq!(theme.selected(), theme.selection);
        }
    }
}
