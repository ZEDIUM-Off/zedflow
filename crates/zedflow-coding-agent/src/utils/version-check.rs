#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LatestPiRelease {
    pub version: String,
    pub package_name: Option<String>,
    pub note: Option<String>,
}
pub fn compare_package_versions(a: &str, b: &str) -> Option<i32> {
    fn p(x: &str) -> Option<Vec<u64>> {
        let x = x.trim().trim_start_matches('v');
        let v = x
            .split('.')
            .map(|s| s.parse().ok())
            .collect::<Option<Vec<_>>>()?;
        Some(v)
    }
    let (a, b) = (p(a)?, p(b)?);
    Some(match a.cmp(&b) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    })
}
pub fn is_newer_package_version(a: &str, b: &str) -> bool {
    compare_package_versions(a, b)
        .map(|x| x > 0)
        .unwrap_or(a.trim() != b.trim())
}
