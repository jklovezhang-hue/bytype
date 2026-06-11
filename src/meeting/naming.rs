/// 由时间零件生成会议基名 `YYYY-MM-DD_HHMMSS`(文件夹名与夹内文件共用此基名)。
pub fn meeting_base_name(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    min: u32,
    sec: u32,
) -> String {
    format!("{year:04}-{month:02}-{day:02}_{hour:02}{min:02}{sec:02}")
}

#[cfg(test)]
mod tests {
    use super::meeting_base_name;

    #[test]
    fn formats_zero_padded() {
        assert_eq!(meeting_base_name(2026, 6, 11, 9, 5, 3), "2026-06-11_090503");
    }

    #[test]
    fn formats_full_width() {
        assert_eq!(meeting_base_name(2026, 12, 31, 23, 59, 59), "2026-12-31_235959");
    }
}
