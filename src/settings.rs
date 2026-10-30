use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    Ru,
    En,
}
impl Language {
    /// The operating system's interface language: Russian for a Russian UI, English otherwise.
    pub fn system() -> Self {
        Self::from_locale(sys_locale::get_locale().as_deref())
    }

    fn from_locale(locale: Option<&str>) -> Self {
        match locale {
            Some(tag) if !tag.to_ascii_lowercase().starts_with("ru") => Self::En,
            _ => Self::Ru,
        }
    }

    pub fn text<'a>(self, ru: &'a str, en: &'a str) -> &'a str {
        if self == Self::Ru { ru } else { en }
    }

    pub fn count(self, count: usize, ru: [&str; 3], en: [&str; 2]) -> String {
        let word = match self {
            Self::Ru => match (count % 100, count % 10) {
                (11..=14, _) => ru[2],
                (_, 1) => ru[0],
                (_, 2..=4) => ru[1],
                _ => ru[2],
            },
            Self::En => en[usize::from(count != 1)],
        };
        // Thousands are split by a narrow no-break space: 6 214.
        let digits = count.to_string();
        let mut number = String::new();
        for (i, ch) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                number.push('\u{202f}');
            }
            number.push(ch);
        }
        format!("{number} {word}")
    }
}

#[cfg(test)]
mod localization_tests {
    use super::Language;

    #[test]
    fn system_language_picks_russian_only_for_russian_locales() {
        assert_eq!(Language::from_locale(Some("ru-RU")), Language::Ru);
        assert_eq!(Language::from_locale(Some("RU")), Language::Ru);
        assert_eq!(Language::from_locale(Some("en-US")), Language::En);
        assert_eq!(Language::from_locale(Some("de-DE")), Language::En);
        assert_eq!(Language::from_locale(None), Language::Ru);
    }

    #[test]
    fn counts_use_russian_and_english_plural_forms() {
        for (count, word) in [
            (0, "скинов"),
            (1, "скин"),
            (2, "скина"),
            (4, "скина"),
            (5, "скинов"),
            (11, "скинов"),
            (12, "скинов"),
            (14, "скинов"),
            (21, "скин"),
            (22, "скина"),
            (25, "скинов"),
            (101, "скин"),
            (111, "скинов"),
            (112, "скинов"),
            (114, "скинов"),
        ] {
            assert_eq!(
                Language::Ru.count(count, ["скин", "скина", "скинов"], ["skin", "skins"]),
                format!("{count} {word}")
            );
        }
        assert_eq!(
            Language::Ru.count(6214, ["файл", "файла", "файлов"], ["file", "files"]),
            "6\u{202f}214 файлов"
        );
        for (count, word) in [(0, "skins"), (1, "skin"), (2, "skins"), (21, "skins")] {
            assert_eq!(
                Language::En.count(count, ["скин", "скина", "скинов"], ["skin", "skins"]),
                format!("{count} {word}")
            );
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub language: Language,
    pub osu_folder: PathBuf,
    pub volume: f32,
    pub ui_scale: f32,
    pub compact: bool,
    pub reduce_motion: bool,
    pub auto_refresh: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            language: Language::Ru,
            osu_folder: PathBuf::new(),
            volume: 0.5,
            ui_scale: 1.0,
            compact: false,
            reduce_motion: false,
            auto_refresh: true,
        }
    }
}
/// `%APPDATA%\Skinweaver`. Settings, history cache and log from the old
/// `osu-skin-mixer-rust` folder move over on first start; if the move fails (an old copy is
/// still running), the old folder keeps being used.
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join("Skinweaver");
    let old = base.join("osu-skin-mixer-rust");
    if !dir.exists() && old.is_dir() && fs::rename(&old, &dir).is_err() {
        return old;
    }
    dir
}

impl Settings {
    pub fn load(data: &Path) -> Result<Self> {
        let file = data.join("settings.json");
        if file.exists() {
            let mut settings: Self = serde_json::from_slice(&fs::read(&file)?)
                .context("Cannot read settings.json. The original file was kept.")?;
            settings.volume = settings.volume.clamp(0.0, 1.0);
            settings.ui_scale = settings.ui_scale.clamp(0.8, 1.6);
            return Ok(settings);
        }
        // First start: speak the system's language until the user picks one in Settings.
        let mut settings = Self {
            language: Language::system(),
            ..Self::default()
        };
        // Import the old folder preference without modifying the Godot settings.
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let legacy = PathBuf::from(appdata).join("osu-skin-mixer/settings.json");
            if legacy.is_file() {
                let value: serde_json::Value = serde_json::from_slice(&fs::read(&legacy)?)
                    .context("Cannot import legacy settings")?;
                if let Some(folder) = value.get("osu_folder").and_then(|v| v.as_str()) {
                    settings.osu_folder = folder.into();
                } else if let Some(folder) = value.get("skins_folder").and_then(|v| v.as_str()) {
                    settings.osu_folder = Path::new(folder)
                        .parent()
                        .unwrap_or(Path::new(folder))
                        .into();
                }
            }
        }
        if settings.osu_folder.as_os_str().is_empty()
            && let Some(local) = std::env::var_os("LOCALAPPDATA")
        {
            let folder = PathBuf::from(local).join("osu!");
            if folder.join("Skins").is_dir() {
                settings.osu_folder = folder;
            }
        }
        Ok(settings)
    }
    pub fn save(&self, data: &Path) -> Result<()> {
        fs::create_dir_all(data)?;
        atomic_write(
            &data.join("settings.json"),
            &serde_json::to_vec_pretty(self)?,
        )
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("Missing parent directory")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path)
        .map_err(|e| e.error)
        .with_context(|| format!("Save {}", path.display()))?;
    Ok(())
}

pub fn instance_lock(data: &Path) -> Result<File> {
    fs::create_dir_all(data)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(data.join("instance.lock"))?;
    if fs2::FileExt::try_lock_exclusive(&file).is_err() {
        bail!("Приложение уже запущено / The app is already running");
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_roundtrip_and_atomic_replace() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = Settings {
            language: Language::En,
            ..Settings::default()
        };
        settings.save(dir.path()).unwrap();
        settings.volume = 0.2;
        settings.save(dir.path()).unwrap();
        let loaded = Settings::load(dir.path()).unwrap();
        assert_eq!(loaded.language, Language::En);
        assert_eq!(loaded.volume, 0.2);
    }
}
