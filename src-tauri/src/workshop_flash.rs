//! Whole-flash factory reinstall for the supported 2 MiB RP2040-Zero.
//! Fill every byte outside the bundled firmware with erased flash (0xff).
pub const FLASH_BYTES: usize = 2 * 1024 * 1024;
const FLASH_BASE: u32 = 0x1000_0000;

pub fn image(uf2: &[u8]) -> Result<Vec<u8>, String> {
    fn word(block: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(block[offset..offset + 4].try_into().unwrap())
    }
    if uf2.is_empty() || uf2.len() % 512 != 0 {
        return Err("The bundled factory firmware is not a complete UF2 image.".into());
    }
    let blocks = uf2.len() / 512;
    let mut image = vec![0xff; FLASH_BYTES];
    let mut occupied = vec![false; FLASH_BYTES];
    for (index, block) in uf2.chunks_exact(512).enumerate() {
        let address = word(block, 12);
        let bytes = word(block, 16) as usize;
        if word(block, 0) != 0x0a32_4655 || word(block, 4) != 0x9e5d_5157
            || word(block, 508) != 0x0ab1_6f30 || word(block, 8) != 0x2000
            || word(block, 28) != 0xe48b_ff56 || word(block, 20) as usize != index
            || word(block, 24) as usize != blocks || bytes != 256
        {
            return Err("The bundled factory firmware has an invalid RP2040 UF2 block.".into());
        }
        let offset = address.checked_sub(FLASH_BASE).ok_or("Factory firmware is outside flash.")? as usize;
        if offset % 256 != 0 || offset.checked_add(bytes).is_none_or(|end| end > FLASH_BYTES)
            || occupied[offset..offset + bytes].iter().any(|used| *used)
        {
            return Err("Factory firmware overlaps or exceeds this board’s flash.".into());
        }
        image[offset..offset + bytes].copy_from_slice(&block[32..32 + bytes]);
        occupied[offset..offset + bytes].fill(true);
    }
    if !occupied[..256].iter().all(|used| *used) {
        return Err("Factory firmware is missing the boot image.".into());
    }
    Ok(image)
}

pub fn require_flash_size(info: &str) -> Result<(), String> {
    let sizes: Vec<_> = info.lines().filter_map(|line| line.split_once(':'))
        .filter(|(key, _)| key.trim().eq_ignore_ascii_case("flash size"))
        .map(|(_, value)| value.trim()).collect();
    if sizes == ["2048K"] { Ok(()) } else {
        Err("Wipe requires a verified RP2040-Zero with 2 MiB flash. Reconnect normally to inspect it; if the flash size cannot be read, use Install firmware first.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const STANDARD: &[u8] = include_bytes!("../../firmware/releases/droidfun-rp2040-zero-gp4-gp5-v1.3.2.uf2");
    const MISTAKE: &[u8] = include_bytes!("../../firmware/releases/droidfun-rp2040-zero-sda5-scl4-v1.3.2-mistake.uf2");
    #[test]
    fn both_factory_images_clear_all_storage_and_keep_exact_firmware_bytes() {
        for uf2 in [STANDARD, MISTAKE] {
            let fresh = image(uf2).unwrap();
            assert_eq!(fresh.len(), FLASH_BYTES);
            let end = uf2.chunks_exact(512).map(|b| {
                let address = u32::from_le_bytes(b[12..16].try_into().unwrap());
                let offset = (address - FLASH_BASE) as usize;
                assert_eq!(&fresh[offset..offset+256], &b[32..288]);
                offset + 256
            }).max().unwrap();
            assert!(fresh[end..].iter().all(|b| *b == 0xff));
            assert!(fresh[FLASH_BYTES - 4096..].iter().all(|b| *b == 0xff));
        }
    }
    #[test]
    fn rejects_corrupt_truncated_overlapping_wrong_family_and_outside_flash_images() {
        assert!(image(&[]).is_err());
        assert!(image(&STANDARD[..STANDARD.len()-1]).is_err());
        for (offset, value) in [(0,0), (8,0), (28,0), (24,0), (12,0x2000_0000)] {
            let mut bad = STANDARD.to_vec();
            bad[offset..offset+4].copy_from_slice(&u32::to_le_bytes(value));
            assert!(image(&bad).is_err());
        }
        let mut overlap = STANDARD.to_vec();
        let first = overlap[12..16].to_vec();
        overlap[524..528].copy_from_slice(&first);
        assert!(image(&overlap).is_err());
    }
    #[test]
    fn rejects_unknown_and_unsupported_flash_sizes_before_writing() {
        assert!(require_flash_size(" flash size: 2048K\n").is_ok());
        for info in ["", "flash size: 1024K", "flash size: 4096K", "flash size: 2048K\nflash size: 2048K"] {
            assert!(require_flash_size(info).is_err());
        }
    }
}
