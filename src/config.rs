use crate::wlr_client::zwlr_layer_surface_v1::{self, Anchor as WlAnchor};
use serde::Deserialize;

#[derive(Deserialize)]
pub enum Anchor {
    Top,
    Bottom,
    Left,
    Right,
}

impl Anchor {
    const VERTICAL: WlAnchor = WlAnchor::from_bits_truncate(WlAnchor::Top.bits() | WlAnchor::Bottom.bits());
    const HORIZONTAL: WlAnchor = WlAnchor::from_bits_truncate(WlAnchor::Left.bits() | WlAnchor::Right.bits());
}

impl Into<WlAnchor> for Anchor {
    fn into(self) -> WlAnchor {
        match self {
            Anchor::Top => Self::HORIZONTAL | WlAnchor::Top,
            Anchor::Bottom => Self::HORIZONTAL | WlAnchor::Bottom,
            Anchor::Left => Self::VERTICAL | WlAnchor::Left,
            Anchor::Right => Self::VERTICAL | WlAnchor::Right,
        }
    }
}

#[derive(Deserialize)]
pub struct Bar {
    pub anchor: Anchor,
    pub height: u32,
    pub width: u32,
    pub foreground: String,
    pub background: String,
}

impl Default for Bar {
    fn default() -> Self {
        Bar {
            anchor: Anchor::Top,
            height: 30,
            width: 1920,
            foreground: "".to_owned(),
            background: "".to_owned(),
        }
    }
}