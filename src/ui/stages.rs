//! Stage-by-stage mixing: the osu! screens in the order a player meets them.
//!
//! Stages refer to catalog nodes by stable keys instead of numeric ids:
//! - `hitcircle` — an image file leaf by its pattern (`selection-mode*` keeps the star);
//! - `snd:normal-hitclap` — a sound leaf by its pattern;
//! - `ini:Colours/MenuGlow` — a skin.ini property;
//! - `grp:Gameplay/osu!/Hitcircles` — any node by its English name path.

use super::editor::image_stem;
use skinweaver::catalog::{OptionNode, Target};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SceneKind {
    Menu,
    SongSelect,
    Mods,
    Play,
    Spinner,
    Break,
    Pause,
    Fail,
    Results,
}

pub(crate) struct SoundGroup {
    pub ru: &'static str,
    pub en: &'static str,
    /// Catalog group shown as one row in the stage tree; `None` lists the leaves there.
    pub group: Option<&'static str>,
    pub leaves: &'static [&'static str],
}

pub(crate) struct StageDef {
    pub code: &'static str,
    pub ru: &'static str,
    pub en: &'static str,
    /// 1 for a sub-stage shown under the previous top-level stage.
    pub depth: u8,
    pub scene: SceneKind,
    /// What the stage tree lists for this stage (scene hotspots may be finer).
    pub items: &'static [&'static str],
    pub sounds: &'static [SoundGroup],
}

const fn sounds(
    ru: &'static str,
    en: &'static str,
    group: Option<&'static str>,
    leaves: &'static [&'static str],
) -> SoundGroup {
    SoundGroup {
        ru,
        en,
        group,
        leaves,
    }
}

/// Hitsound sets offered as a fork in the gameplay stage.
pub(crate) const HIT_SETS: [&str; 3] = ["normal", "soft", "drum"];

pub(crate) const STAGES: &[StageDef] = &[
    StageDef {
        code: "A",
        ru: "Главное меню",
        en: "Main menu",
        depth: 0,
        scene: SceneKind::Menu,
        items: &[
            "menu-background",
            "welcome_text",
            "menu-snow",
            "ini:Colours/MenuGlow",
        ],
        sounds: &[sounds(
            "Меню",
            "Menu",
            None,
            &[
                "snd:welcome",
                "snd:seeya",
                "snd:heartbeat",
                "snd:menu-play-*",
                "snd:menu-edit-*",
                "snd:menu-options-*",
                "snd:menu-exit-*",
                "snd:menu-freeplay-*",
                "snd:menu-multiplayer-*",
                "snd:menuhit",
                "snd:click-short",
                "snd:click-short-confirm",
            ],
        )],
    },
    StageDef {
        code: "B",
        ru: "Выбор карты",
        en: "Song select",
        depth: 0,
        scene: SceneKind::SongSelect,
        items: &[
            "songselect-top",
            "songselect-bottom",
            "menu-button-background",
            "ini:Colours/SongSelectActiveText",
            "ini:Colours/SongSelectInactiveText",
            "selection-tab",
            "star",
            "menu-back",
            "grp:Interface/Song select/Bottom bar buttons",
            "mode-osu",
            "mode-osu-small",
            "grp:Interface/Song select/Ranking icons",
            "scoreentry-*",
            "grp:Interface/Mod icons",
        ],
        sounds: &[sounds(
            "Выбор карты",
            "Song select",
            None,
            &[
                "snd:menuclick",
                "snd:select-difficulty",
                "snd:select-expand",
                "snd:click-short",
                "snd:click-short-confirm",
                "snd:back-button-hover",
                "snd:back-button-click",
                "snd:menu-back-*",
                "snd:menuback",
            ],
        )],
    },
    StageDef {
        code: "B1",
        ru: "Выбор модов",
        en: "Mod select",
        depth: 1,
        scene: SceneKind::Mods,
        items: &["grp:Interface/Mod icons"],
        sounds: &[sounds(
            "Моды",
            "Mods",
            None,
            &["snd:check-on", "snd:check-off"],
        )],
    },
    StageDef {
        code: "C",
        ru: "Геймплей",
        en: "Gameplay",
        depth: 0,
        scene: SceneKind::Play,
        items: &[
            "grp:Gameplay/osu!/Hitcircles",
            "grp:Gameplay/osu!/Sliders",
            "grp:Gameplay/osu!/Combo Colours",
            "followpoint",
            "grp:Gameplay/osu!/Hit judgements",
            "grp:Cursor/Head",
            "grp:Cursor/Trail",
            "grp:Interface/In-game/Health bar",
            "grp:Interface/In-game/Fonts",
            "grp:Interface/In-game/Key overlay",
            "scoreentry-*",
            "play-skip",
        ],
        sounds: &[
            sounds(
                "Normal",
                "Normal",
                Some("grp:Hitsounds/Normal"),
                &[
                    "snd:normal-hitnormal",
                    "snd:normal-hitclap",
                    "snd:normal-hitwhistle",
                    "snd:normal-hitfinish",
                    "snd:normal-slidertick",
                    "snd:normal-sliderslide",
                    "snd:normal-sliderwhistle",
                ],
            ),
            sounds(
                "Soft",
                "Soft",
                Some("grp:Hitsounds/Soft"),
                &[
                    "snd:soft-hitnormal",
                    "snd:soft-hitclap",
                    "snd:soft-hitwhistle",
                    "snd:soft-hitfinish",
                    "snd:soft-slidertick",
                    "snd:soft-sliderslide",
                    "snd:soft-sliderwhistle",
                ],
            ),
            sounds(
                "Drum",
                "Drum",
                Some("grp:Hitsounds/Drum"),
                &[
                    "snd:drum-hitnormal",
                    "snd:drum-hitclap",
                    "snd:drum-hitwhistle",
                    "snd:drum-hitfinish",
                    "snd:drum-slidertick",
                    "snd:drum-sliderslide",
                    "snd:drum-sliderwhistle",
                ],
            ),
            sounds(
                "Общие",
                "Shared",
                None,
                &["snd:combobreak", "snd:comboburst"],
            ),
        ],
    },
    StageDef {
        code: "C1",
        ru: "Спиннер",
        en: "Spinner",
        depth: 1,
        scene: SceneKind::Spinner,
        items: &["grp:Gameplay/osu!/Spinners", "spinner-rpm"],
        sounds: &[sounds(
            "Спиннер",
            "Spinner",
            Some("grp:Hitsounds/Spinner"),
            &["snd:spinnerspin", "snd:spinnerbonus"],
        )],
    },
    StageDef {
        code: "C2",
        ru: "Отсчёт и перерывы",
        en: "Countdown and breaks",
        depth: 1,
        scene: SceneKind::Break,
        items: &[
            "grp:Interface/In-game/Countdown",
            "section-pass",
            "section-fail",
            "play-warningarrow",
            "arrow-warning",
        ],
        sounds: &[
            sounds(
                "Отсчёт",
                "Countdown",
                Some("grp:Menu Sounds/In-game/Countdown"),
                &[
                    "snd:readys",
                    "snd:count3s",
                    "snd:count2s",
                    "snd:count1s",
                    "snd:gos",
                ],
            ),
            sounds(
                "Перерыв",
                "Break",
                Some("grp:Menu Sounds/In-game/Section pass/fail"),
                &["snd:sectionpass", "snd:sectionfail"],
            ),
        ],
    },
    StageDef {
        code: "D",
        ru: "Пауза",
        en: "Pause",
        depth: 0,
        scene: SceneKind::Pause,
        items: &[
            "pause-overlay",
            "pause-continue",
            "pause-retry",
            "pause-back",
            "arrow-pause",
        ],
        sounds: &[sounds(
            "Пауза",
            "Pause",
            None,
            &[
                "snd:pause-loop",
                "snd:pause-hover",
                "snd:pause-continue-*",
                "snd:pause-retry-*",
                "snd:pause-back-*",
            ],
        )],
    },
    StageDef {
        code: "D1",
        ru: "Неудачная попытка",
        en: "Fail",
        depth: 1,
        scene: SceneKind::Fail,
        items: &[
            "fail-background",
            "pause-retry",
            "pause-back",
            "arrow-pause",
        ],
        sounds: &[sounds(
            "Неудача",
            "Fail",
            None,
            &[
                "snd:failsound",
                "snd:pause-hover",
                "snd:pause-retry-*",
                "snd:pause-back-*",
            ],
        )],
    },
    StageDef {
        code: "E",
        ru: "Результаты",
        en: "Results",
        depth: 0,
        scene: SceneKind::Results,
        items: &[
            "ranking-panel",
            "ranking-title",
            "grp:Interface/Results screen/Grades",
            "ranking-accuracy",
            "ranking-maxcombo",
            "ranking-graph",
            "ranking-perfect",
            "ranking-retry",
            "ranking-replay",
            "grp:Gameplay/osu!/Hit judgements",
            "grp:Interface/In-game/Fonts",
            "grp:Interface/Mod icons",
            "menu-back",
        ],
        sounds: &[sounds(
            "Результаты",
            "Results",
            None,
            &["snd:applause", "snd:click-short-confirm", "snd:menuback"],
        )],
    },
];

/// A row of the mod select panel: label ru, label en, (mod file suffix, abbreviation).
pub(crate) type ModRow = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);

pub(crate) const MOD_ROWS: &[ModRow] = &[
    (
        "Облегчение",
        "Difficulty reduction",
        &[("easy", "EZ"), ("nofail", "NF"), ("halftime", "HT")],
    ),
    (
        "Усложнение",
        "Difficulty increase",
        &[
            ("hardrock", "HR"),
            ("suddendeath", "SD"),
            ("perfect", "PF"),
            ("doubletime", "DT"),
            ("nightcore", "NC"),
            ("hidden", "HD"),
            ("fadein", "FI"),
            ("flashlight", "FL"),
        ],
    ),
    (
        "Особые",
        "Special",
        &[
            ("relax", "RX"),
            ("relax2", "AP"),
            ("spunout", "SO"),
            ("autoplay", "AT"),
            ("cinema", "CN"),
            ("target", "TP"),
            ("scorev2", "V2"),
            ("touchdevice", "TD"),
        ],
    ),
    (
        "osu!mania",
        "osu!mania",
        &[
            ("key1", "1K"),
            ("key2", "2K"),
            ("key3", "3K"),
            ("key4", "4K"),
            ("key5", "5K"),
            ("key6", "6K"),
            ("key7", "7K"),
            ("key8", "8K"),
            ("key9", "9K"),
            ("keycoop", "CO"),
            ("random", "RD"),
            ("mirror", "MR"),
        ],
    ),
];

/// How a variant tile draws a node: several files layered or side by side.
#[derive(Clone, Copy)]
pub(crate) enum Look {
    /// Layers centred on each other, bottom first.
    Stack(&'static [&'static str]),
    /// Layers aligned to the top-left corner, bottom first.
    Corner(&'static [&'static str]),
    /// Files side by side.
    Row(&'static [&'static str]),
    /// skin.ini colour swatches.
    Colors(&'static [&'static str]),
}

const LOOKS: &[(&str, Look)] = &[
    (
        "grp:Gameplay/osu!/Hitcircles",
        Look::Stack(&["hitcircle", "hitcircleoverlay", "default-1"]),
    ),
    (
        "grp:Gameplay/osu!/Sliders",
        Look::Stack(&["sliderfollowcircle", "sliderb"]),
    ),
    (
        "grp:Gameplay/osu!/Hit judgements",
        Look::Row(&["hit300", "hit100", "hit50", "hit0"]),
    ),
    (
        "grp:Gameplay/osu!/Spinners",
        Look::Stack(&[
            "spinner-circle",
            "spinner-bottom",
            "spinner-top",
            "spinner-middle2",
            "spinner-middle",
        ]),
    ),
    (
        "grp:Gameplay/osu!/Combo Colours",
        Look::Colors(&["Combo1", "Combo2", "Combo3", "Combo4", "Combo5"]),
    ),
    ("grp:Cursor/Head", Look::Stack(&["cursor"])),
    (
        "grp:Cursor/Trail",
        Look::Row(&["cursortrail", "cursormiddle"]),
    ),
    (
        "grp:Interface/In-game/Health bar",
        Look::Corner(&["scorebar-bg", "scorebar-colour"]),
    ),
    (
        "grp:Interface/In-game/Fonts",
        Look::Row(&["score-1", "score-2", "score-3", "score-x"]),
    ),
    (
        "scoreentry-*",
        Look::Row(&["scoreentry-1", "scoreentry-2", "scoreentry-3"]),
    ),
    (
        "grp:Interface/In-game/Key overlay",
        Look::Row(&["inputoverlay-key", "inputoverlay-key"]),
    ),
    (
        "grp:Interface/In-game/Countdown",
        Look::Row(&["count3", "count2", "count1"]),
    ),
    (
        "grp:Interface/Song select/Bottom bar buttons",
        Look::Row(&[
            "selection-mode",
            "selection-mods",
            "selection-random",
            "selection-options",
        ]),
    ),
    (
        "grp:Interface/Song select/Ranking icons",
        Look::Row(&["ranking-x-small", "ranking-s-small", "ranking-a-small"]),
    ),
    (
        "grp:Interface/Song select/Button",
        Look::Row(&["button-left", "button-middle", "button-right"]),
    ),
    (
        "grp:Interface/Mod icons",
        Look::Row(&[
            "selection-mod-hidden",
            "selection-mod-hardrock",
            "selection-mod-doubletime",
        ]),
    ),
    (
        "grp:Interface/Results screen/Grades",
        Look::Row(&["ranking-x", "ranking-s", "ranking-a"]),
    ),
];

/// Lookup tables between stage keys and catalog ids.
#[derive(Default)]
pub(crate) struct Index {
    ids: HashMap<String, String>,
    animated: HashMap<String, bool>,
    looks: HashMap<String, Look>,
}

impl Index {
    pub fn build(nodes: &[OptionNode]) -> Self {
        fn walk(nodes: &[OptionNode], prefix: &str, path: &str, index: &mut Index) {
            for (i, node) in nodes.iter().enumerate() {
                let id = format!("{prefix}{i}");
                let name_path = if path.is_empty() {
                    node.name.clone()
                } else {
                    format!("{path}/{}", node.name)
                };
                index
                    .ids
                    .entry(format!("grp:{name_path}"))
                    .or_insert_with(|| id.clone());
                match &node.target {
                    Some(target @ Target::File { pattern, audio, .. }) => {
                        let key = if *audio {
                            format!("snd:{pattern}")
                        } else {
                            pattern.clone()
                        };
                        index.ids.entry(key).or_insert_with(|| id.clone());
                        if let Some((stem, animated)) = image_stem(target) {
                            index.animated.insert(stem, animated);
                        }
                    }
                    Some(Target::Property { section, key }) => {
                        index
                            .ids
                            .entry(format!("ini:{section}/{key}"))
                            .or_insert_with(|| id.clone());
                    }
                    _ => {}
                }
                walk(&node.children, &format!("{id}."), &name_path, index);
            }
        }
        let mut index = Self::default();
        walk(nodes, "", "", &mut index);
        for (key, look) in LOOKS {
            if let Some(id) = index.ids.get(*key).cloned() {
                index.looks.insert(id, *look);
            }
        }
        index
    }

    /// Catalog id for a stage key.
    pub fn id(&self, key: &str) -> Option<&str> {
        self.ids.get(key).map(String::as_str)
    }

    pub fn animated(&self, stem: &str) -> bool {
        self.animated.get(stem).copied().unwrap_or(false)
    }

    pub fn look(&self, id: &str) -> Option<Look> {
        self.looks.get(id).copied()
    }
}

/// Every stage key, elements and sounds alike.
pub(crate) fn stage_keys(stage: &StageDef) -> impl Iterator<Item = &'static str> + '_ {
    stage.items.iter().copied().chain(
        stage
            .sounds
            .iter()
            .flat_map(|g| g.group.into_iter().chain(g.leaves.iter().copied())),
    )
}

/// Codes of the other stages where an element (or a part of it) shows up too.
pub(crate) fn also_on(index: &Index, id: &str, current: usize) -> Vec<&'static str> {
    STAGES
        .iter()
        .enumerate()
        .filter(|(i, stage)| {
            *i != current
                && stage_keys(stage)
                    .filter_map(|k| index.id(k))
                    .any(|other| related(other, id))
        })
        .map(|(_, stage)| stage.code)
        .collect()
}

/// Ids related by nesting: a group and anything inside it.
pub(crate) fn related(a: &str, b: &str) -> bool {
    let nested = |outer: &str, inner: &str| {
        inner == outer
            || inner
                .strip_prefix(outer)
                .is_some_and(|r| r.starts_with('.'))
    };
    nested(a, b) || nested(b, a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use skinweaver::catalog::catalog;

    #[test]
    fn every_stage_key_resolves() {
        let index = Index::build(&catalog());
        for stage in STAGES {
            for key in stage.items {
                assert!(index.id(key).is_some(), "{}: {key}", stage.code);
            }
            for group in stage.sounds {
                for key in group.leaves.iter().chain(group.group.iter()) {
                    assert!(index.id(key).is_some(), "{}: {key}", stage.code);
                }
            }
        }
        for (key, _) in LOOKS {
            assert!(index.id(key).is_some(), "look {key}");
        }
        assert_eq!(index.id("snd:comboburst"), Some("3.7"));
        assert_ne!(index.id("comboburst"), index.id("snd:comboburst"));
        assert!(index.animated("menu-back"));
        assert!(!index.animated("hitcircle"));
        // Shared elements are offered on every stage where they are seen.
        let rank = index.id("grp:Interface/Song select/Ranking icons").unwrap();
        assert!(also_on(&index, rank, 0).contains(&"B"));
        let fonts = index.id("grp:Interface/In-game/Fonts").unwrap();
        assert_eq!(also_on(&index, fonts, 3), ["B", "E"]);
        let retry = index.id("pause-retry").unwrap();
        assert_eq!(also_on(&index, retry, 6), ["D1"]);
        // Every mod icon of the catalog has its own place on the mod select panel.
        let mods: Vec<_> = MOD_ROWS.iter().flat_map(|(_, _, m)| m.iter()).collect();
        assert_eq!(mods.len(), 31);
        for (name, _) in mods {
            assert!(
                index.id(&format!("selection-mod-{name}")).is_some(),
                "{name}"
            );
        }
    }

    #[test]
    fn nesting_is_by_whole_segments() {
        assert!(related("1.0", "1.0.2"));
        assert!(related("1.0.2", "1.0"));
        assert!(!related("1.0", "1.01"));
        assert!(!related("1.1", "1.0"));
    }
}
