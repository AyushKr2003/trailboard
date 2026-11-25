//! This process's memory, read from `/proc/self` (for the script's `mem`
//! line and the benchmark).

/// Fields of `/proc/self/smaps_rollup` in KiB: (RSS, PSS, private).
fn rollup(text: &str) -> (u64, u64, u64) {
    let (mut rss, mut pss, mut private) = (0, 0, 0);
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else { continue };
        let kib: u64 = value.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
        match key {
            "Rss" => rss = kib,
            "Pss" => pss = kib,
            "Private_Clean" | "Private_Dirty" => private += kib,
            _ => {}
        }
    }
    (rss, pss, private)
}

/// "rss 312.4 MiB pss 290.1 MiB private 270.0 MiB", or why it couldn't be read.
pub fn summary() -> String {
    match std::fs::read_to_string("/proc/self/smaps_rollup") {
        Ok(text) => {
            let (rss, pss, private) = rollup(&text);
            let mib = |k: u64| k as f64 / 1024.0;
            format!("rss {:.1} MiB pss {:.1} MiB private {:.1} MiB", mib(rss), mib(pss), mib(private))
        }
        Err(e) => format!("unreadable ({e})"),
    }
}

#[cfg(test)]
mod tests {
    use super::rollup;

    #[test]
    fn reads_the_rollup_fields() {
        let text = "55d0-7ffd ---p 00000000 00:00 0  [rollup]\n\
                    Rss:              300000 kB\n\
                    Pss:              250000 kB\n\
                    Pss_Anon:         200000 kB\n\
                    Shared_Clean:      40000 kB\n\
                    Private_Clean:     10000 kB\n\
                    Private_Dirty:    200000 kB\n";
        assert_eq!(rollup(text), (300000, 250000, 210000));
    }
}
