//! Java simple case folding for scoreboard ordering (no expanding Unicode folds).
use std::{cmp::Ordering, io::Read, sync::OnceLock};

fn fold(ch: char) -> u32 {
    static FOLDS: OnceLock<Vec<(u32, u32)>> = OnceLock::new();
    let pairs = FOLDS.get_or_init(|| {
        #[derive(serde::Deserialize)]
        struct Data {
            folds: Vec<(u32, u32)>,
        }
        let mut json = Vec::new();
        flate2::read::GzDecoder::new(&include_bytes!("../../data/26.2/java-case-fold.json.gz")[..])
            .read_to_end(&mut json)
            .expect("bundled Java case fold data");
        serde_json::from_slice::<Data>(&json)
            .expect("bundled Java case fold JSON")
            .folds
    });
    pairs
        .binary_search_by_key(&(ch as u32), |p| p.0)
        .map_or(ch as u32, |i| pairs[i].1)
}
pub(super) fn compare(a: &str, b: &str) -> Ordering {
    a.chars().map(fold).cmp(b.chars().map(fold))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comparisons_match_java_including_non_expanding_and_supplementary_case() {
        let mut json = Vec::new();
        flate2::read::GzDecoder::new(&include_bytes!("../../data/26.2/java-case-fold.json.gz")[..])
            .read_to_end(&mut json)
            .unwrap();
        let data: serde_json::Value = serde_json::from_slice(&json).unwrap();
        for row in data["comparisons"].as_array().unwrap() {
            let a = row[0].as_str().unwrap();
            let b = row[1].as_str().unwrap();
            assert_eq!(
                compare(a, b) as i32,
                row[2].as_i64().unwrap() as i32,
                "{a:?} vs {b:?}"
            );
        }
    }
}
