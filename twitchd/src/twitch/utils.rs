use super::types::{StreamIndex, PlaylistInfo, Quality, ApproxQuality};

pub fn find_playlist(index: StreamIndex, quality: &Quality)
    -> Option<PlaylistInfo>
{
    use self::Quality::{Exact, Approx};

    match quality {
        Exact(exact_quality) => find_playlist_exact(index, exact_quality),
        Approx(approx_quality) => find_playlist_approx(index, approx_quality)
    }
}

fn find_playlist_exact(index: StreamIndex, quality: &str)
    -> Option<PlaylistInfo>
{
    index.playlist_infos.into_iter()
        .find(|info| info.media_info.name.to_lowercase() == quality)
}

fn find_playlist_approx(mut index: StreamIndex, quality: &ApproxQuality)
    -> Option<PlaylistInfo>
{
    let as_slice = index.playlist_infos.as_mut_slice();
    as_slice.sort_by_key(|info| info.stream_info.bandwidth);

    let playlist_ref = match *quality {
        ApproxQuality::Best => as_slice.last(),
        ApproxQuality::Worst => as_slice.first(),
    };

    playlist_ref.cloned()
}

impl From<String> for Quality {
    fn from(value: String) -> Quality {
        match &value.to_lowercase() as &str {
            "best"  => Quality::Approx(ApproxQuality::Best),
            "worst" => Quality::Approx(ApproxQuality::Worst),
            quality => Quality::Exact(String::from(quality)),
        }
    }
}

impl Default for Quality {
    fn default() -> Self {
        Quality::Approx(ApproxQuality::Best)
    }
}
