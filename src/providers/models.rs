use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ProviderKind {
    #[default]
    #[serde(rename = "moviebox", alias = "movie_box")]
    MovieBox,
    #[serde(rename = "fourkhdhub", alias = "four_k_hd_hub", alias = "4khdhub")]
    FourKHdHub,
    #[serde(rename = "bdix_circleftp", alias = "bdix_circle_ftp")]
    BdixCircleFtp,
    #[serde(rename = "bdix_dhakaflix", alias = "bdix_dhaka_flix")]
    BdixDhakaFlix,
    #[serde(rename = "addons", alias = "addon")]
    Addons,
    #[serde(rename = "dramachi")]
    Dramachi,
}

impl ProviderKind {
    pub const ENABLED: [Self; 5] = [
        Self::MovieBox,
        Self::FourKHdHub,
        Self::Dramachi,
        Self::BdixCircleFtp,
        Self::BdixDhakaFlix,
    ];

    pub const fn cache_key(self) -> &'static str {
        match self {
            Self::MovieBox => "moviebox",
            Self::FourKHdHub => "fourkhdhub",
            Self::BdixCircleFtp => "bdix_circleftp",
            Self::BdixDhakaFlix => "bdix_dhakaflix",
            Self::Addons => "addons",
            Self::Dramachi => "dramachi",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::MovieBox => "MovieBox",
            Self::FourKHdHub => "4KHDHub",
            Self::BdixCircleFtp => "CircleFTP (BDIX)",
            Self::BdixDhakaFlix => "DhakaFlix (BDIX)",
            Self::Addons => "Addons",
            Self::Dramachi => "Dramachi",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "moviebox" => Some(Self::MovieBox),
            "4khdhub" | "fourkhdhub" => Some(Self::FourKHdHub),
            "bdix_circleftp" | "circleftp (bdix)" => Some(Self::BdixCircleFtp),
            "bdix_dhakaflix" | "dhakaflix (bdix)" => Some(Self::BdixDhakaFlix),
            "addons" | "addon" => Some(Self::Addons),
            "dramachi" => Some(Self::Dramachi),
            _ => None,
        }
    }

    pub const fn is_bdix(self) -> bool {
        matches!(self, Self::BdixCircleFtp | Self::BdixDhakaFlix)
    }
}

impl fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderMediaId {
    pub provider: ProviderKind,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestContext {
    pub provider: ProviderKind,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaType {
    Movie,
    Series,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogItem {
    pub id: ProviderMediaId,
    pub title: String,
    pub media_type: MediaType,
    pub year: Option<String>,
    pub poster_url: Option<String>,
    pub season_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Episode {
    pub season: usize,
    pub number: usize,
    pub title: Option<String>,
    pub overview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Season {
    pub number: usize,
    pub episodes: Vec<Episode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioTrackOption {
    pub subject_id: String,
    pub language: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaDetails {
    pub id: ProviderMediaId,
    pub title: String,
    pub media_type: MediaType,
    pub year: Option<String>,
    pub description: Option<String>,
    pub tagline: Option<String>,
    pub imdb_rating: Option<String>,
    pub director: Option<String>,
    pub stars: Option<String>,
    pub prints: Option<String>,
    pub audios: Option<String>,
    pub poster_url: Option<String>,
    pub duration: Option<String>,
    pub genres: Vec<String>,
    pub seasons: Vec<Season>,
    pub dubs: Vec<AudioTrackOption>,
}

impl MediaDetails {
    pub fn is_series(&self) -> bool {
        self.media_type == MediaType::Series || !self.seasons.is_empty()
    }

    pub fn has_languages(&self) -> bool {
        self.dubs.len() > 1
    }

    pub fn cover_url(&self) -> Option<&str> {
        self.poster_url.as_deref()
    }

    pub fn from_search_result(
        item: &crate::models::SearchResult,
        preview: Option<&MediaDetails>,
    ) -> Self {
        if let Some(p) = preview.filter(|p| p.id.value == item.id && p.id.provider == item.provider)
        {
            let mut details = p.clone();
            if details.title.trim().is_empty() {
                details.title = item.title.clone();
            }
            if details.year.is_none() && !item.release_year.trim().is_empty() {
                details.year = Some(item.release_year.clone());
            }
            if details.poster_url.is_none() {
                details.poster_url = item.cover_url.clone();
            }
            details
        } else {
            MediaDetails {
                id: ProviderMediaId {
                    provider: item.provider,
                    value: item.id.clone(),
                },
                title: item.title.clone(),
                media_type: if item.stype == 2 {
                    MediaType::Series
                } else {
                    MediaType::Movie
                },
                year: if !item.release_year.trim().is_empty() {
                    Some(item.release_year.clone())
                } else {
                    None
                },
                description: None,
                tagline: None,
                imdb_rating: None,
                director: None,
                stars: None,
                prints: None,
                audios: None,
                poster_url: item.cover_url.clone(),
                duration: None,
                genres: vec![],
                seasons: vec![],
                dubs: vec![],
            }
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionIntent {
    Playback,
    Download,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceMirror {
    pub label: String,
    pub resolver_url: String,
    pub headers: Vec<(String, String)>,
    pub direct_file: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtitleOption {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    pub provider: ProviderKind,
    pub filename: String,
    pub quality: Option<String>,
    pub codec: Option<String>,
    pub language: Option<String>,
    pub size_bytes: Option<u64>,
    pub season: Option<usize>,
    pub episode: Option<usize>,
    pub mirrors: Vec<SourceMirror>,
    #[serde(default)]
    pub resource_id: Option<String>,
}
impl Release {
    pub fn is_multi_resolution(&self) -> bool {
        self.quality
            .as_deref()
            .is_some_and(|q| q.eq_ignore_ascii_case("multi") || q.eq_ignore_ascii_case("multi-res"))
    }

    pub fn resolution_u64(&self) -> u64 {
        self.quality
            .as_deref()
            .and_then(|q| {
                let trimmed = q.trim();
                if trimmed.eq_ignore_ascii_case("4k") || trimmed.eq_ignore_ascii_case("uhd") {
                    return Some(2160);
                }
                trimmed.trim_end_matches(['p', 'P']).parse::<u64>().ok()
            })
            .unwrap_or(1080)
    }

    pub fn resolution_i64(&self) -> i64 {
        if self.is_multi_resolution() {
            -1
        } else {
            self.resolution_u64() as i64
        }
    }
    pub fn source_label(&self) -> &str {
        self.mirrors
            .first()
            .map(|m| m.label.as_str())
            .unwrap_or_else(|| match self.provider {
                ProviderKind::FourKHdHub => "4KHDHub",
                ProviderKind::BdixCircleFtp => "CircleFTP",
                ProviderKind::BdixDhakaFlix => "DhakaFlix",
                ProviderKind::Addons => "Addon",
                ProviderKind::Dramachi => "Dramachi",
                ProviderKind::MovieBox => "Direct",
            })
    }

    pub fn direct_url(&self) -> Option<&str> {
        self.mirrors.first().map(|m| m.resolver_url.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackSource {
    pub provider: ProviderKind,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub subtitle: Option<String>,
    pub source_label: String,
    #[serde(default)]
    pub max_height: Option<u64>,
}

impl PlaybackSource {
    pub fn bare(provider: ProviderKind, url: impl Into<String>, subtitle: Option<String>) -> Self {
        Self {
            provider,
            url: url.into(),
            headers: Vec::new(),
            subtitle,
            source_label: provider.label().to_string(),
            max_height: None,
        }
    }
}
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderError {
    #[error("Network connection failed: {0}")]
    Network(String),
    #[error("Rate limited by provider")]
    RateLimited(Option<u64>),
    #[error("Item not found on provider")]
    NotFound,
    #[error("Failed to parse response: {0}")]
    Parsing(String),
    #[error("Provider is temporarily unavailable: {0}")]
    Unavailable(String),
}

impl ProviderError {
    pub fn user_message(&self, provider: ProviderKind) -> String {
        let label = match provider {
            ProviderKind::BdixCircleFtp => "CircleFTP",
            ProviderKind::BdixDhakaFlix => "DhakaFlix",
            _ => provider.label(),
        };

        match self {
            Self::Network(msg) => {
                if provider.is_bdix() {
                    format!("{label} unreachable: requires BDIX network.")
                } else {
                    let lower = msg.to_ascii_lowercase();
                    if lower.contains("timed out") || lower.contains("timeout") {
                        format!("{label} timed out.")
                    } else {
                        format!("Cannot reach {label}.")
                    }
                }
            }
            Self::RateLimited(secs) => match secs {
                Some(s) => format!("Rate limited. Wait {s}s."),
                None => "Rate limited. Try later.".to_string(),
            },
            Self::NotFound => "No results found.".to_string(),
            Self::Parsing(_) => format!("{label} parse error."),
            Self::Unavailable(msg) => {
                let trimmed = msg.trim();
                if let Some(status) = trimmed.strip_prefix("HTTP status ") {
                    format!("{label} error ({status}).")
                } else if trimmed.is_empty() {
                    format!("{label} unavailable.")
                } else {
                    format!("{label} unavailable: {trimmed}")
                }
            }
        }
    }
}

impl From<String> for ProviderError {
    fn from(msg: String) -> Self {
        Self::Unavailable(msg)
    }
}

impl From<&str> for ProviderError {
    fn from(msg: &str) -> Self {
        Self::Unavailable(msg.to_string())
    }
}

pub fn strip_emojis(input: &str) -> String {
    input
        .chars()
        .filter(|&c| {
            let u = c as u32;
            !((0x1F000..=0x1FAFF).contains(&u)
                || (0x2600..=0x27BF).contains(&u)
                || (0x2300..=0x23FF).contains(&u)
                || (0x2B00..=0x2BFF).contains(&u)
                || (0xFE00..=0xFE0F).contains(&u)
                || u == 0x200D)
        })
        .collect::<String>()
}

pub fn clean_stream_text(input: &str) -> String {
    let without_emojis = strip_emojis(input);
    let mut cleaned = String::new();
    let mut last_was_space = false;
    for c in without_emojis.chars() {
        if c.is_whitespace() {
            if !last_was_space && !cleaned.is_empty() {
                cleaned.push(' ');
                last_was_space = true;
            }
        } else {
            cleaned.push(c);
            last_was_space = false;
        }
    }
    cleaned.trim().to_string()
}

pub fn extract_4digit_year(raw: &str) -> String {
    raw.as_bytes()
        .windows(4)
        .find(|window| window.iter().all(u8::is_ascii_digit) && matches!(window[0], b'1' | b'2'))
        .and_then(|window| std::str::from_utf8(window).ok())
        .map(str::to_string)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_release_resolution_parsing() {
        let make_release = |q: Option<&str>| Release {
            provider: ProviderKind::BdixCircleFtp,
            filename: "Test.mkv".to_string(),
            quality: q.map(|s| s.to_string()),
            codec: None,
            language: None,
            size_bytes: None,
            season: None,
            episode: None,
            mirrors: Vec::new(),
            resource_id: None,
        };

        assert_eq!(make_release(Some("4K")).resolution_u64(), 2160);
        assert_eq!(make_release(Some("4k")).resolution_u64(), 2160);
        assert_eq!(make_release(Some("2160p")).resolution_u64(), 2160);
        assert_eq!(make_release(Some("1080p")).resolution_u64(), 1080);
        assert_eq!(make_release(None).resolution_u64(), 1080);
    }

    #[test]
    fn test_provider_error_user_message_concise() {
        let bdix_net_err = ProviderError::Network("tcp connect error: operation timed out".into());
        assert_eq!(
            bdix_net_err.user_message(ProviderKind::BdixCircleFtp),
            "CircleFTP unreachable: requires BDIX network."
        );
        assert_eq!(
            bdix_net_err.user_message(ProviderKind::BdixDhakaFlix),
            "DhakaFlix unreachable: requires BDIX network."
        );

        let timeout_err = ProviderError::Network("operation timed out".into());
        assert_eq!(
            timeout_err.user_message(ProviderKind::MovieBox),
            "MovieBox timed out."
        );

        let connect_err = ProviderError::Network("dns lookup failed".into());
        assert_eq!(
            connect_err.user_message(ProviderKind::FourKHdHub),
            "Cannot reach 4KHDHub."
        );

        let rate_limit = ProviderError::RateLimited(Some(30));
        assert_eq!(
            rate_limit.user_message(ProviderKind::MovieBox),
            "Rate limited. Wait 30s."
        );

        let not_found = ProviderError::NotFound;
        assert_eq!(
            not_found.user_message(ProviderKind::MovieBox),
            "No results found."
        );

        let http_status = ProviderError::Unavailable("HTTP status 502".into());
        assert_eq!(
            http_status.user_message(ProviderKind::FourKHdHub),
            "4KHDHub error (502)."
        );
    }
}
