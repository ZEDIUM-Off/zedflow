use std::time::Duration;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LatestPiRelease {
    pub version: String,
    pub package_name: Option<String>,
    pub note: Option<String>,
}
pub fn compare_package_versions(a: &str, b: &str) -> Option<i32> {
    let parse = |v: &str| {
        let v = v.trim().trim_start_matches('v');
        let mut p = v.splitn(2, '-');
        let nums: Vec<u64> = p
            .next()?
            .split('.')
            .map(|x| x.parse().ok())
            .collect::<Option<_>>()?;
        if nums.len() != 3 {
            return None;
        }
        let pre = p.next().map(|s| {
            s.split('.')
                .map(|x| {
                    if let Ok(n) = x.parse::<u64>() {
                        (0, n.to_string())
                    } else {
                        (1, x.to_owned())
                    }
                })
                .collect::<Vec<_>>()
        });
        Some((nums, pre))
    };
    let (an, ap) = parse(a)?;
    let (bn, bp) = parse(b)?;
    for (x, y) in an.iter().zip(bn.iter()) {
        if x != y {
            return Some(x.cmp(y) as i32);
        }
    }
    Some(match (ap, bp) {
        (None, None) => 0,
        (None, Some(_)) => 1,
        (Some(_), None) => -1,
        (Some(a), Some(b)) => a
            .iter()
            .zip(b.iter())
            .find_map(|(x, y)| {
                if x != y {
                    Some(if x.0 != y.0 {
                        x.0.cmp(&y.0) as i32
                    } else {
                        if let (Ok(a), Ok(b)) = (x.1.parse::<u64>(), y.1.parse::<u64>()) {
                            a.cmp(&b) as i32
                        } else {
                            x.1.cmp(&y.1) as i32
                        }
                    })
                } else {
                    None
                }
            })
            .unwrap_or(a.len().cmp(&b.len()) as i32),
    })
}
pub fn is_newer_package_version(candidate: &str, current: &str) -> bool {
    compare_package_versions(candidate, current)
        .map_or(candidate.trim() != current.trim(), |x| x > 0)
}
pub fn get_latest_pi_release(
    current: &str,
    timeout: Option<Duration>,
) -> Result<Option<LatestPiRelease>, Box<dyn std::error::Error>> {
    if std::env::var_os("PI_SKIP_VERSION_CHECK").is_some()
        || std::env::var_os("PI_OFFLINE").is_some()
    {
        return Ok(None);
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout.or(Some(Duration::from_secs(10))))
        .build()?;
    let v: serde_json::Value = client
        .get("https://pi.dev/api/latest-version")
        .header(
            "User-Agent",
            crate::utils::pi_user_agent::get_pi_user_agent(current),
        )
        .header("accept", "application/json")
        .send()?
        .json()?;
    let Some(version) = v["version"]
        .as_str()
        .map(str::trim)
        .filter(|x| !x.is_empty())
    else {
        return Ok(None);
    };
    Ok(Some(LatestPiRelease {
        version: version.to_owned(),
        package_name: v["packageName"]
            .as_str()
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .map(str::to_owned),
        note: v["note"]
            .as_str()
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .map(str::to_owned),
    }))
}
pub fn check_for_new_pi_version(current: &str) -> Option<LatestPiRelease> {
    get_latest_pi_release(current, None)
        .ok()
        .flatten()
        .filter(|x| is_newer_package_version(&x.version, current))
}
