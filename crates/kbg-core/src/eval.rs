//! The detector on the real packs, for tuning (ignored test). For pairs of installed layouts, the
//! 50 000 most used words of each language (wordfreq), weighted by use, are typed in their own
//! layout (any switch is false) and with the other layout on (should switch back).
//!
//! Needs `target/packs/<code>/` (kbg-pack-builder output, unzipped) and
//! `target/pack-data/eval-<code>.txt` (`word frequency` lines); see CLAUDE.md.
//! `cargo test --release -p kbg-core eval -- --ignored --nocapture`

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use crate::{Detector, Key, LayoutId, Layouts, Pack, Stroke};

/// The text keys, in the order of the tables below: digits row, Q row, A row and `` ` ``, the key
/// left of Enter, Z row.
fn scans() -> Vec<u16> {
    (0x02..=0x0D)
        .chain(0x10..=0x1B)
        .chain(0x1E..=0x29)
        .chain([0x2B])
        .chain(0x2C..=0x35)
        .collect()
}

/// Unshifted characters per layout; `\0` = dead key. Capitals come from Shift as uppercase.
const LAYOUTS: &[(&str, usize, &str)] = &[
    ("en", 0x0409_0409, "1234567890-=qwertyuiop[]asdfghjkl;'`\\zxcvbnm,./"),
    ("ru", 0x0419_0419, "1234567890-=йцукенгшщзхъфывапролджэё\\ячсмитьбю."),
    ("es", 0x040A_040A, "1234567890'¡qwertyuiop\0+asdfghjklñ\0ºçzxcvbnm,.-"),
    ("fr", 0x040C_040C, "&é\"'(-è_çà)=azertyuiop\0$qsdfghjklmù²*wxcvbn,;:!"),
    ("de", 0x0407_0407, "1234567890ß\0qwertzuiopü+asdfghjklöä\0#yxcvbnm,.-"),
    // Polish (Programmers) types US letters; its own letters need AltGr.
    ("pl", 0x0415_0415, "1234567890-=qwertyuiop[]asdfghjkl;'`\\zxcvbnm,./"),
];

struct Tables {
    installed: Vec<LayoutId>,
}

fn table(layout: LayoutId) -> Vec<char> {
    let (.., keys) = LAYOUTS.iter().find(|l| l.1 == layout.0).unwrap();
    keys.chars().collect()
}

impl Layouts for Tables {
    fn current(&self) -> Option<LayoutId> {
        self.installed.first().copied()
    }

    fn installed(&self) -> Vec<LayoutId> {
        self.installed.clone()
    }

    fn render(&self, layout: LayoutId, strokes: &[Stroke]) -> Option<String> {
        let (scans, keys) = (scans(), table(layout));
        strokes
            .iter()
            .map(|s| {
                let c = keys[scans.iter().position(|&k| k == s.key.scan)?];
                match c {
                    '\0' => None,
                    _ if s.shift => c.to_uppercase().next(),
                    _ => Some(c),
                }
            })
            .collect()
    }

    fn stroke(&self, layout: LayoutId, c: char) -> Option<Stroke> {
        let (scans, keys) = (scans(), table(layout));
        let i = keys.iter().position(|&k| k == c && k != '\0')?;
        Some(Stroke {
            key: Key::new(scans[i]),
            shift: false,
            caps: false,
            altgr: false,
        })
    }
}

fn target() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target")
}

fn load(code: &str) -> Arc<Pack> {
    let dir = target().join("packs").join(code);
    let pack = Pack::new(
        &fs::read_to_string(dir.join("pack.toml")).unwrap(),
        crate::Bytes::new(fs::read(dir.join("words.fst")).unwrap()),
        crate::Bytes::new(fs::read(dir.join("trigrams.bin")).unwrap()),
    );
    Arc::new(pack.unwrap())
}

fn id(code: &str) -> LayoutId {
    LayoutId(LAYOUTS.iter().find(|l| l.0 == code).unwrap().1)
}

#[test]
#[ignore = "needs the built packs; see the module comment"]
fn eval() {
    let pairs = [
        ("en", "ru"),
        ("en", "fr"),
        ("en", "de"),
        ("en", "es"),
        ("en", "pl"),
        ("ru", "fr"),
        ("ru", "de"),
        ("ru", "es"),
        ("ru", "pl"),
        ("fr", "de"),
    ];
    for (a, b) in pairs {
        let detector = Detector::new(vec![(id(a), load(a)), (id(b), load(b))]);
        let layouts = Tables {
            installed: vec![id(a), id(b)],
        };
        for (code, other) in [(a, b), (b, a)] {
            let (here, there) = (id(code), id(other));
            let list = fs::read_to_string(target().join(format!("pack-data/eval-{code}.txt")))
                .unwrap();
            let (mut total, mut long, mut false_switch, mut caught) = (0.0, 0.0, 0.0, 0.0);
            let mut wrong = Vec::new();
            for line in list.lines() {
                let (word, f) = line.split_once(' ').unwrap();
                let f: f64 = f.parse().unwrap();
                let keys: Option<Vec<Stroke>> =
                    word.chars().map(|c| layouts.stroke(here, c)).collect();
                // Not typeable without Shift or dead keys here.
                let Some(keys) = keys else {
                    continue;
                };
                total += f;
                if detector.detect(&keys, here, &layouts).is_some() {
                    false_switch += f;
                    wrong.push(word);
                }
                // Only words that look different in the other layout can be caught.
                let differs = layouts.render(there, &keys).is_some_and(|t| t != word);
                if keys.len() >= 3 && differs {
                    long += f;
                    if detector.detect(&keys, there, &layouts) == Some(here) {
                        caught += f;
                    }
                }
            }
            println!(
                "{a}+{b} {code}: false {:.3}%  caught {:.1}%  false top: {:?}",
                100.0 * false_switch / total,
                100.0 * caught / long.max(f64::MIN_POSITIVE),
                &wrong[..wrong.len().min(8)]
            );
        }
    }
}
