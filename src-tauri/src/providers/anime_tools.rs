use crate::models::MediaMatch;
use animedb::{MediaKind, RemoteApi, SearchOptions};
use std::sync::LazyLock;

static ANILIST: LazyLock<RemoteApi> = LazyLock::new(RemoteApi::anilist);

fn canonical_to_media_match(
    media: &animedb::CanonicalMedia,
    provider_name: &str,
) -> Option<MediaMatch> {
    let source_id = media
        .external_ids
        .iter()
        .find(|id| id.source.as_str() == provider_name)
        .map(|id| id.source_id.clone())
        .or_else(|| {
            media
                .external_ids
                .first()
                .map(|id| id.source_id.clone())
        })?;

    Some(MediaMatch {
        provider: provider_name.to_string(),
        source_id,
        title_display: media.title_display.clone(),
        title_native: media.title_native.clone(),
        title_english: media.title_english.clone(),
        title_romaji: media.title_romaji.clone(),
        synopsis: media.synopsis.clone(),
        genres: media.genres.clone(),
        tags: media.tags.clone(),
        aliases: media.aliases.clone(),
        format: media.format.clone(),
        episodes: media.episodes,
        season: media.season.clone(),
        season_year: media.season_year,
    })
}

pub fn search_anime(query: &str, limit: usize) -> Result<Vec<MediaMatch>, String> {
    search_anime_combined(query, limit, None)
}

pub fn search_anime_combined(
    query: &str,
    limit: usize,
    tmdb_api_key: Option<&str>,
) -> Result<Vec<MediaMatch>, String> {
    let opts = SearchOptions::default()
        .with_limit(limit)
        .with_media_kind(MediaKind::Anime);

    let mut matches: Vec<MediaMatch> = Vec::new();

    if let Ok(results) = ANILIST.search(query, opts.clone()) {
        for m in &results {
            if let Some(mm) = canonical_to_media_match(m, "AniList") {
                matches.push(mm);
            }
        }
    }

    if let Some(key) = tmdb_api_key.filter(|k| !k.trim().is_empty()) {
        let tmdb_provider = animedb::TmdbProvider::new().with_api_key(key);
        let tmdb_api = RemoteApi::with_provider(tmdb_provider);

        if let Ok(results) = tmdb_api.search(query, opts) {
            for m in &results {
                if let Some(mm) = canonical_to_media_match(m, "TMDB") {
                    if !matches.iter().any(|e| e.title_display.eq_ignore_ascii_case(&mm.title_display)) {
                        matches.push(mm);
                    }
                }
            }
        }
    }

    Ok(matches)
}

pub fn get_anime_by_id(source_id: &str) -> Result<Option<MediaMatch>, String> {
    get_anime_by_id_with_provider("AniList", source_id, None)
}

pub fn get_anime_by_id_with_provider(
    provider_name: &str,
    source_id: &str,
    tmdb_api_key: Option<&str>,
) -> Result<Option<MediaMatch>, String> {
    if provider_name == "TMDB" {
        let key = tmdb_api_key.unwrap_or_default();
        if key.trim().is_empty() {
            return Ok(None);
        }
        let tmdb_provider = animedb::TmdbProvider::new().with_api_key(key);
        let tmdb_api = RemoteApi::with_provider(tmdb_provider);
        let media = tmdb_api
            .anime_metadata()
            .by_id(source_id)
            .map_err(|e| format!("TMDB lookup by ID failed: {}", e))?;
        Ok(media.and_then(|m| canonical_to_media_match(&m, "TMDB")))
    } else {
        let media = ANILIST
            .anime_metadata()
            .by_id(source_id)
            .map_err(|e| format!("AniList lookup by ID failed: {}", e))?;

        Ok(media.and_then(|m| canonical_to_media_match(&m, "AniList")))
    }
}

pub async fn fetch_anilist_characters(source_id: &str) -> Vec<crate::models::CharacterProfile> {
    let Ok(id) = source_id.parse::<i32>() else {
        return Vec::new();
    };

    let query = r#"
        query ($id: Int) {
          Media (id: $id, type: ANIME) {
            characters (sort: [ROLE, RELEVANCE], perPage: 15) {
              edges {
                role
                node {
                  name {
                    full
                    userPreferred
                  }
                  description
                }
              }
            }
          }
        }
    "#;

    let body = serde_json::json!({
        "query": query,
        "variables": { "id": id }
    });

    let client = reqwest::Client::new();
    let res = match client
        .post("https://graphql.anilist.co")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let json: serde_json::Value = match res.json().await {
        Ok(j) => j,
        Err(_) => return Vec::new(),
    };

    let mut result = Vec::new();
    if let Some(edges) = json["data"]["Media"]["characters"]["edges"].as_array() {
        for edge in edges {
            let role = edge["role"].as_str().map(|r| r.to_lowercase());
            let name_full = edge["node"]["name"]["userPreferred"]
                .as_str()
                .or_else(|| edge["node"]["name"]["full"].as_str())
                .unwrap_or("")
                .to_string();

            if name_full.is_empty() {
                continue;
            }

            let raw_desc = edge["node"]["description"].as_str().unwrap_or("");
            let desc_cleaned = clean_anilist_description(raw_desc);
            let description = if desc_cleaned.is_empty() {
                None
            } else {
                Some(desc_cleaned)
            };

            result.push(crate::models::CharacterProfile {
                name: name_full,
                role,
                description,
                relationships: None,
                verified: true,
            });
        }
    }

    result
}

fn clean_anilist_description(raw: &str) -> String {
    let spoiler_regex = regex::Regex::new(r"(?s)~!.*?!~").unwrap();
    let cleaned = spoiler_regex.replace_all(raw, "");
    let link_regex = regex::Regex::new(r"\[([^\]]+)\]\([^)]+\)").unwrap();
    let cleaned = link_regex.replace_all(&cleaned, "$1");
    cleaned.trim().chars().take(300).collect()
}

#[cfg(test)]
mod test_tmdb {
    use super::*;
    use animedb::TmdbProvider;

    #[test]
    fn check_tmdb_api() {
        let provider = TmdbProvider::new().with_api_key("test_key");
        let _api = RemoteApi::with_provider(provider);
    }
}
