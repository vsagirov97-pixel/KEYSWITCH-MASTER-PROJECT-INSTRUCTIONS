//! Tray icon states and the embedded `.ico` data (pure code, compiled on every target so the
//! parser is tested on the host).
//!
//! The icons are generated in-repo by `assets/gen_tray_icons.py` (original glyph, no
//! third-party art) and embedded with `include_bytes!`: no resource compiler, no new crate.

/// What the tray icon shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IconState {
    /// Hotkey registered, not paused.
    #[default]
    Active,
    /// Paused from the tray menu.
    Paused,
    /// Needs attention: the hotkey could not be registered (conflict) or another error.
    Attention,
}

impl IconState {
    /// The embedded `.ico` file of this state.
    pub fn ico(self) -> &'static [u8] {
        match self {
            Self::Active => include_bytes!("../assets/tray-active.ico"),
            Self::Paused => include_bytes!("../assets/tray-paused.ico"),
            Self::Attention => include_bytes!("../assets/tray-attention.ico"),
        }
    }

    /// Every state, in a fixed order.
    pub const ALL: [IconState; 3] = [Self::Active, Self::Paused, Self::Attention];
}

/// The image of an `.ico` file closest to `size` pixels (the smallest one not below it, else
/// the largest): its bytes (a DIB or PNG, as `CreateIconFromResourceEx` takes them) and its
/// width. `None` if the file is malformed.
pub fn pick_image(ico: &[u8], size: u32) -> Option<(&[u8], u32)> {
    let u16_at = |at: usize| -> Option<u16> {
        Some(u16::from_le_bytes(ico.get(at..at + 2)?.try_into().ok()?))
    };
    let u32_at = |at: usize| -> Option<u32> {
        Some(u32::from_le_bytes(ico.get(at..at + 4)?.try_into().ok()?))
    };
    if u16_at(0)? != 0 || u16_at(2)? != 1 {
        return None;
    }
    let count = usize::from(u16_at(4)?);
    let mut best: Option<(&[u8], u32)> = None;
    for i in 0..count {
        let entry = 6 + 16 * i;
        // A width byte of 0 means 256 pixels.
        let width = match *ico.get(entry)? {
            0 => 256,
            w => u32::from(w),
        };
        let len = usize::try_from(u32_at(entry + 8)?).ok()?;
        let offset = usize::try_from(u32_at(entry + 12)?).ok()?;
        let data = ico.get(offset..offset.checked_add(len)?)?;
        let better = match best {
            None => true,
            Some((_, b)) if b < size => width > b,
            Some((_, b)) => width >= size && width < b,
        };
        if better {
            best = Some((data, width));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_state_has_all_sizes_as_32_bit_dibs() {
        for state in IconState::ALL {
            for want in [16, 20, 24, 32, 48] {
                let (data, width) = pick_image(state.ico(), want).expect("well-formed icon");
                assert_eq!(width, want);
                // BITMAPINFOHEADER: size 40, width, height doubled (XOR + AND), 32 bpp.
                let w = i32::from_le_bytes(data[4..8].try_into().unwrap());
                let h = i32::from_le_bytes(data[8..12].try_into().unwrap());
                let bpp = u16::from_le_bytes(data[14..16].try_into().unwrap());
                assert_eq!((data[0], w, h, bpp), (40, want as i32, 2 * want as i32, 32));
                let stride = (want as usize).div_ceil(32) * 4;
                let n = want as usize;
                assert_eq!(data.len(), 40 + n * n * 4 + n * stride);
            }
        }
    }

    #[test]
    fn closest_size_is_picked() {
        let ico = IconState::Active.ico();
        assert_eq!(pick_image(ico, 18).map(|(_, w)| w), Some(20));
        assert_eq!(pick_image(ico, 40).map(|(_, w)| w), Some(48));
        assert_eq!(pick_image(ico, 64).map(|(_, w)| w), Some(48));
        assert_eq!(pick_image(ico, 1).map(|(_, w)| w), Some(16));
    }

    #[test]
    fn states_differ_and_malformed_data_is_rejected() {
        assert_ne!(IconState::Active.ico(), IconState::Paused.ico());
        assert_ne!(IconState::Paused.ico(), IconState::Attention.ico());
        assert!(pick_image(&[], 16).is_none());
        assert!(pick_image(&[0, 0, 2, 0, 1, 0], 16).is_none());
        // One entry pointing past the end of the file.
        let mut bad = vec![0, 0, 1, 0, 1, 0, 16, 16, 0, 0, 1, 0, 32, 0];
        bad.extend(100u32.to_le_bytes());
        bad.extend(22u32.to_le_bytes());
        assert!(pick_image(&bad, 16).is_none());
    }
}
