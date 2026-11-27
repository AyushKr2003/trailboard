//! Which GPU Chromium decodes video on (VA-API).
//!
//! Chromium's VA-API opens the first render node, `/dev/dri/renderD128`, and
//! gives up if that one is NVIDIA's proprietary driver (it has no VA-API).
//! On a laptop with an Intel or AMD iGPU and an NVIDIA dGPU, NVIDIA's driver
//! often takes the first node, and then H.264, HEVC, VP9 and AV1 lose their
//! hardware decoder (HEVC stops playing at all: Chromium has no software HEVC
//! decoder). So when the first node is NVIDIA's, Trailboard points Chromium
//! at the first one that isn't (`--render-node-override`), unless the
//! command line already names one.

use std::path::Path;

/// The kernel driver of each render node, by number (`renderD128` → 128).
pub fn render_nodes(dri_class: &Path) -> Vec<(u32, String)> {
    let Ok(entries) = std::fs::read_dir(dri_class) else { return Vec::new() };
    let mut nodes: Vec<(u32, String)> = entries
        .flatten()
        .filter_map(|e| {
            let n: u32 = e.file_name().to_str()?.strip_prefix("renderD")?.parse().ok()?;
            let driver = std::fs::read_link(e.path().join("device/driver")).ok()?;
            Some((n, driver.file_name()?.to_str()?.to_string()))
        })
        .collect();
    nodes.sort();
    nodes
}

/// Drivers Chromium's VA-API skips (no VA-API of their own).
fn skipped(driver: &str) -> bool {
    driver == "nvidia"
}

/// The render node Chromium should use instead of its default, if any:
/// `None` when the first node is usable (or none is).
pub fn override_for(nodes: &[(u32, String)]) -> Option<String> {
    let (_, first) = nodes.first()?;
    if !skipped(first) {
        return None;
    }
    nodes.iter().find(|(_, d)| !skipped(d)).map(|(n, _)| format!("/dev/dri/renderD{n}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nodes(list: &[(u32, &str)]) -> Vec<(u32, String)> {
        list.iter().map(|&(n, d)| (n, d.to_string())).collect()
    }

    #[test]
    fn skips_nvidia_on_the_first_node_only() {
        // Intel iGPU alone, or first: Chromium's own choice is right.
        assert_eq!(override_for(&nodes(&[(128, "i915")])), None);
        assert_eq!(override_for(&nodes(&[(128, "i915"), (129, "nvidia")])), None);
        assert_eq!(override_for(&nodes(&[(128, "amdgpu"), (129, "nvidia")])), None);
        // NVIDIA's driver took the first node: use the iGPU's.
        assert_eq!(override_for(&nodes(&[(128, "nvidia"), (129, "i915")])).as_deref(), Some("/dev/dri/renderD129"));
        assert_eq!(override_for(&nodes(&[(128, "nvidia"), (129, "xe")])).as_deref(), Some("/dev/dri/renderD129"));
        // Only NVIDIA, or nothing: nothing better to offer.
        assert_eq!(override_for(&nodes(&[(128, "nvidia")])), None);
        assert_eq!(override_for(&[]), None);
        // Nouveau has VA-API (through Mesa): Chromium may use it.
        assert_eq!(override_for(&nodes(&[(128, "nouveau"), (129, "i915")])), None);
    }

    #[test]
    fn reads_nodes_in_number_order() {
        let dir = tempfile::tempdir().unwrap();
        let mk = |name: &str, driver: &str| {
            let dev = dir.path().join(name).join("device");
            std::fs::create_dir_all(&dev).unwrap();
            std::os::unix::fs::symlink(format!("../../../bus/pci/drivers/{driver}"), dev.join("driver")).unwrap();
        };
        mk("renderD129", "i915");
        mk("renderD128", "nvidia");
        mk("card1", "nvidia");
        std::fs::create_dir(dir.path().join("renderD130")).unwrap(); // no driver: skipped
        assert_eq!(render_nodes(dir.path()), nodes(&[(128, "nvidia"), (129, "i915")]));
        assert_eq!(render_nodes(&dir.path().join("missing")), Vec::new());
    }
}
