use crate::tray::model::SlotAnchor;

#[derive(Clone, Copy)]
pub(crate) struct PopupRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub(crate) fn place_root(
    anchor: &SlotAnchor,
    desired: (f32, f32),
    size: (f32, f32),
    bottom: bool,
) -> PopupRect {
    let width = desired.0.min((size.0 - 16.0).max(0.0));
    let edge = if bottom {
        anchor.y - 4.0
    } else {
        anchor.y + anchor.height + 4.0
    };
    let available = if bottom {
        edge - 8.0
    } else {
        size.1 - edge - 8.0
    };
    let height = desired.1.min(available.max(0.0));
    PopupRect {
        x: anchor.x.clamp(8.0, (size.0 - width - 8.0).max(8.0)),
        y: if bottom { edge - height } else { edge },
        width,
        height,
    }
}
pub(crate) fn place_submenu(row: PopupRect, desired: (f32, f32), size: (f32, f32)) -> PopupRect {
    let width = desired.0.min((size.0 - 16.0).max(0.0));
    let height = desired.1.min((size.1 - 16.0).max(0.0));
    let right = row.x + row.width + 4.0;
    let x = if right + width <= size.0 - 8.0 {
        right
    } else {
        row.x - width - 4.0
    };
    PopupRect {
        x: x.clamp(8.0, (size.0 - width - 8.0).max(8.0)),
        y: row.y.clamp(8.0, (size.1 - height - 8.0).max(8.0)),
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn popup_geometry_clamps_and_flips() {
        let anchor = SlotAnchor {
            output: "DP-9".into(),
            x: 780.0,
            y: 40.0,
            width: 26.0,
            height: 26.0,
        };
        let root = place_root(&anchor, (240.0, 300.0), (800.0, 600.0), false);
        assert!(root.x >= 0.0 && root.x + root.width <= 800.0);
        assert!(root.y >= anchor.y + anchor.height);
        let bottom = SlotAnchor { y: 560.0, ..anchor };
        let root = place_root(&bottom, (240.0, 300.0), (800.0, 600.0), true);
        assert!(root.y + root.height <= bottom.y);
        let row = PopupRect {
            x: 560.0,
            y: 580.0,
            width: 230.0,
            height: 30.0,
        };
        let sub = place_submenu(row, (220.0, 200.0), (800.0, 600.0));
        assert!(sub.x < row.x);
        assert!(sub.y + sub.height <= 600.0);
    }
}
