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
    let opts = SearchOptions::default()
        .with_limit(limit)
        .with_media_kind(MediaKind::Anime);

    let results = ANILIST
        .search(query, opts)
        .map_err(|e| format!("AniList search failed: {}", e))?;

    let matches: Vec<MediaMatch> = results
        .iter()
        .filter_map(|m| canonical_to_media_match(m, "AniList"))
        .collect();

    Ok(matches)
}

pub fn get_anime_by_id(source_id: &str) -> Result<Option<MediaMatch>, String> {
    let media = ANILIST
        .anime_metadata()
        .by_id(source_id)
        .map_err(|e| format!("AniList lookup by ID failed: {}", e))?;

    Ok(media.and_then(|m| canonical_to_media_match(&m, "AniList")))
}
