//! Prefer latest-frame presentation without allowing scanout tearing.
//! Mailbox decouples rendering from scanout. FIFO is the universally supported
//! tear-free fallback; it can throttle rendering to the display refresh rate.
pub fn select(vsync: bool, modes: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    use wgpu::PresentMode::*;
    if !vsync && modes.contains(&Mailbox) {
        Mailbox
    } else {
        Fifo
    }
}
pub fn label(vsync: bool, mode: wgpu::PresentMode) -> &'static str {
    if vsync {
        "On"
    } else if mode == wgpu::PresentMode::Mailbox {
        "Off / Mailbox"
    } else {
        "Off / FIFO fallback"
    }
}
pub fn description(vsync: bool, mode: wgpu::PresentMode) -> &'static str {
    if vsync {
        "Вывод кадров: FIFO"
    } else if mode == wgpu::PresentMode::Mailbox {
        "Вывод кадров: Mailbox, без разрывов"
    } else {
        "Mailbox недоступен: FIFO, предел частоты экрана"
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::PresentMode::*;
    #[test]
    fn never_falls_back_to_a_tearing_mode_including_on_moltenvk() {
        let candidates = [Fifo, FifoRelaxed, Immediate, Mailbox];
        for mask in 0..16 {
            let modes: Vec<_> = candidates
                .iter()
                .enumerate()
                .filter_map(|(i, m)| (mask & (1 << i) != 0).then_some(*m))
                .collect();
            assert_eq!(select(true, &modes), Fifo);
            assert_eq!(
                select(false, &modes),
                if modes.contains(&Mailbox) {
                    Mailbox
                } else {
                    Fifo
                }
            );
        }
        assert_eq!(select(false, &[Fifo, Immediate]), Fifo);
    }
    #[test]
    fn fallback_is_disclosed_and_is_not_reported_as_unrestricted_off() {
        assert!(label(false, Fifo).contains("fallback"));
        assert!(description(false, Fifo).contains("предел частоты экрана"));
        assert_eq!(label(false, Mailbox), "Off / Mailbox");
        assert_eq!(label(true, Fifo), "On");
    }
}
