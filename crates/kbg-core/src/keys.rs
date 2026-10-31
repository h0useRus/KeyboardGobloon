/// A physical key: its set 1 scan code and whether it has the E0 prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub scan: u16,
    pub extended: bool,
}

impl Key {
    pub const fn new(scan: u16) -> Self {
        Self {
            scan,
            extended: false,
        }
    }

    /// The keys whose character depends on the layout: letters, digits and punctuation
    /// (rows `1`..`=`, `Q`..`]`, `A`..`` ` ``, `\`..`/` and the 102nd key next to left Shift).
    pub fn is_text(self) -> bool {
        !self.extended
            && matches!(
                self.scan,
                0x02..=0x0D | 0x10..=0x1B | 0x1E..=0x29 | 0x2B..=0x35 | 0x56
            )
    }
}

/// One typed character: the key and the state that chose which of its characters came out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stroke {
    pub key: Key,
    pub shift: bool,
    pub caps: bool,
    pub altgr: bool,
}

impl Stroke {
    pub const fn plain(scan: u16) -> Self {
        Self {
            key: Key::new(scan),
            shift: false,
            caps: false,
            altgr: false,
        }
    }
}

/// An installed keyboard layout, as the platform names it (on Windows the HKL).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayoutId(pub usize);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_keys() {
        assert!(Key::new(0x10).is_text()); // Q
        assert!(Key::new(0x29).is_text()); // `
        assert!(Key::new(0x56).is_text()); // 102nd key
        assert!(!Key::new(0x39).is_text()); // Space
        assert!(!Key::new(0x1C).is_text()); // Enter
        assert!(!Key::new(0x2A).is_text()); // left Shift
        assert!(
            !Key {
                scan: 0x35,
                extended: true
            }
            .is_text()
        ); // numpad /
    }
}
