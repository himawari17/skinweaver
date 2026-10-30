use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Debug, Deserialize)]
pub struct OptionNode {
    pub name: String,
    #[serde(default)]
    pub ru: String,
    #[serde(default)]
    pub preview: Vec<String>,
    #[serde(default)]
    pub children: Vec<OptionNode>,
    pub target: Option<Target>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Target {
    File {
        pattern: String,
        audio: bool,
        animated: bool,
        suffixes: Option<Vec<String>>,
    },
    Property {
        section: String,
        key: String,
    },
    Section {
        section: String,
        key: String,
        value: String,
    },
}

impl OptionNode {
    pub fn label(&self, russian: bool) -> &str {
        if russian && !self.ru.is_empty() {
            &self.ru
        } else {
            &self.name
        }
    }
    pub fn matches_query(&self, query: &str, russian: bool) -> bool {
        self.label(russian).to_lowercase().contains(query)
            || self.name.to_lowercase().contains(query)
            || self
                .children
                .iter()
                .any(|child| child.matches_query(query, russian))
    }
    pub fn leaves(&self) -> Vec<&Self> {
        if self.target.is_some() {
            return vec![self];
        }
        self.children.iter().flat_map(Self::leaves).collect()
    }
}

/// The bundled catalog, parsed once.
pub fn catalog_ref() -> &'static [OptionNode] {
    static CATALOG: std::sync::OnceLock<Vec<OptionNode>> = std::sync::OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../data/catalog.json")).expect("bundled catalog")
    })
}

pub fn catalog() -> Vec<OptionNode> {
    catalog_ref().to_vec()
}

/// `text` without `prefix`, compared ignoring ASCII case; no allocation.
fn strip_prefix_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

impl Target {
    /// Whether a file belongs to this element. Runs for every file of every source and for
    /// every sprite of a scene, so it compares in place instead of lower-casing copies.
    pub fn matches(&self, path: &Path) -> bool {
        let Self::File {
            pattern,
            audio,
            animated,
            suffixes,
        } = self
        else {
            return false;
        };
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let allowed: &[&str] = if *audio {
            &["wav", "ogg", "mp3"]
        } else {
            &["png", "jpg", "jpeg"]
        };
        if !allowed.iter().any(|a| ext.eq_ignore_ascii_case(a)) {
            return false;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let stem = match stem.len().checked_sub(3) {
            Some(cut)
                if stem
                    .get(cut..)
                    .is_some_and(|t| t.eq_ignore_ascii_case("@2x")) =>
            {
                &stem[..cut]
            }
            _ => stem,
        };
        if let Some(prefix) = pattern.strip_suffix('*') {
            return strip_prefix_ci(stem, prefix).is_some_and(|tail| {
                suffixes
                    .as_ref()
                    .is_none_or(|s| s.iter().any(|s| s.eq_ignore_ascii_case(tail)))
            });
        }
        if stem.eq_ignore_ascii_case(pattern) {
            return true;
        }
        *animated
            && strip_prefix_ci(stem, pattern).is_some_and(|tail| {
                let tail = tail.strip_prefix('-').unwrap_or(tail);
                !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translated_catalog_searches_groups_and_original_names() {
        let nodes = catalog();
        let cursor = &nodes[2];
        assert_eq!(cursor.label(true), "Курсор");
        assert_eq!(cursor.children[0].label(true), "Основной курсор");
        assert_eq!(cursor.children[0].label(false), "Head");
        assert!(cursor.matches_query("основной курсор", true));
        assert!(cursor.matches_query("head", true));
        assert!(cursor.matches_query("cursor.png", true));
        assert!(nodes[1].matches_query("слайдеры", true));
        assert!(nodes[1].matches_query("sliders", true));
        assert!(!cursor.matches_query("слайдеры", true));
        fn check_labels(nodes: &[OptionNode], mods: bool) {
            for node in nodes {
                assert!(!node.label(true).is_empty());
                assert!(
                    mods || !node.ru.is_empty()
                        || node.name.starts_with("osu!")
                        || node.name == "Pippidon"
                        || node.name.chars().next().is_some_and(|c| c.is_ascii_digit()),
                    "Missing Russian label: {}",
                    node.name
                );
                check_labels(&node.children, node.name == "Mod icons");
            }
        }
        check_labels(&nodes, false);
    }
    #[test]
    fn catalog_is_complete_and_animations_match() {
        let nodes = catalog();
        assert_eq!(nodes.len(), 5);
        assert_eq!(nodes.iter().flat_map(OptionNode::leaves).count(), 333);
        let target = Target::File {
            pattern: "menu-back".into(),
            audio: false,
            animated: true,
            suffixes: None,
        };
        assert!(target.matches(Path::new("MENU-BACK-12@2x.PNG")));
        assert!(!target.matches(Path::new("menu-back-click.wav")));
        assert!(!target.matches(Path::new("menu-back-foo.png")));
        let target = Target::File {
            pattern: "selection-mode*".into(),
            audio: false,
            animated: false,
            suffixes: Some(vec!["".into(), "-over".into()]),
        };
        assert!(target.matches(Path::new("selection-mode-over@2x.png")));
        assert!(!target.matches(Path::new("selection-mode-wrong.png")));
    }
}
