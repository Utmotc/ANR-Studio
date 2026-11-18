use std::fs;
use std::io;
use std::path::Path;

/// INI 文档：保留原文件的换行风格与 UTF-8 BOM，写入用临时文件替换。
pub struct IniDoc {
    pub lines: Vec<String>,
    nl: &'static str,
    bom: bool,
}

impl IniDoc {
    pub fn load(path: &Path) -> Self {
        let bytes = fs::read(path).unwrap_or_default();
        let bom = bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF;
        let start = if bom { 3 } else { 0 };
        let text = String::from_utf8_lossy(&bytes[start..]).to_string();
        let crlf = text.contains("\r\n");
        let nl = if crlf { "\r\n" } else { "\n" };
        let lines: Vec<String> = if crlf {
            text.split("\r\n").map(|s| s.to_string()).collect()
        } else {
            text.split('\n').map(|s| s.to_string()).collect()
        };
        Self { lines, nl, bom }
    }

    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(|l| l.trim().is_empty())
    }

    pub fn get(&self, section: &str, key: &str) -> Option<String> {
        let mut in_section = false;
        for line in &self.lines {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix('[') {
                if let Some(name) = rest.split(']').next() {
                    in_section = name.trim().eq_ignore_ascii_case(section);
                    continue;
                }
            }
            if in_section {
                if let Some((k, v)) = t.split_once('=') {
                    if k.trim().eq_ignore_ascii_case(key) {
                        return Some(v.trim().to_string());
                    }
                }
            }
        }
        None
    }

    pub fn set(&mut self, section: &str, key: &str, value: &str) {
        let mut in_section = false;
        let mut section_end: Option<usize> = None;
        let mut key_line: Option<usize> = None;

        for (i, line) in self.lines.iter().enumerate() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix('[') {
                if in_section && section_end.is_none() {
                    section_end = Some(i);
                }
                if let Some(name) = rest.split(']').next() {
                    in_section = name.trim().eq_ignore_ascii_case(section);
                }
                continue;
            }
            if in_section {
                if let Some((k, _)) = t.split_once('=') {
                    if k.trim().eq_ignore_ascii_case(key) {
                        key_line = Some(i);
                        break;
                    }
                }
            }
        }

        if let Some(i) = key_line {
            self.lines[i] = format!("{}={}", key, value);
            return;
        }

        let new_line = format!("{}={}", key, value);
        match section_end {
            Some(mut idx) => {
                // 插到本节最后一个非空行之后
                while idx > 0 && self.lines[idx - 1].trim().is_empty() {
                    idx -= 1;
                }
                self.lines.insert(idx, new_line);
            }
            None => {
                if !self.lines.is_empty() && !self.lines.last().map(|l| l.trim().is_empty()).unwrap_or(true) {
                    self.lines.push(String::new());
                }
                self.lines.push(format!("[{}]", section));
                self.lines.push(new_line);
            }
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let body = self.lines.join(self.nl);
        let mut bytes: Vec<u8> = Vec::with_capacity(body.len() + 3);
        if self.bom {
            bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        }
        bytes.extend_from_slice(body.as_bytes());

        let tmp = path.with_extension("ini.anr-tmp");
        fs::write(&tmp, &bytes)?;
        // Windows: rename 覆盖已存在文件是原子替换
        match fs::rename(&tmp, path) {
            Ok(()) => Ok(()),
            Err(e) => {
                let _ = fs::remove_file(&tmp);
                Err(e)
            }
        }
    }

    /// 写入前把原文件备份成 .bak（只在第一次写时创建，避免覆盖用户手里的原始备份）
    pub fn backup_once(path: &Path) {
        let bak = path.with_extension("ini.anr-bak");
        if !bak.exists() && path.exists() {
            let _ = fs::copy(path, &bak);
        }
    }

    /// 扁平 ini（无 section）读键
    pub fn get_flat(&self, key: &str) -> Option<String> {
        for line in &self.lines {
            let t = line.trim_start();
            if t.starts_with(';') || t.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = t.split_once('=') {
                if k.trim().eq_ignore_ascii_case(key) {
                    return Some(v.trim().to_string());
                }
            }
        }
        None
    }

    /// 扁平 ini（无 section）写键，保持注释原样
    pub fn set_flat(&mut self, key: &str, value: &str) {
        for line in self.lines.iter_mut() {
            let t = line.trim_start();
            if t.starts_with(';') || t.starts_with('#') {
                continue;
            }
            if let Some((k, _)) = t.split_once('=') {
                if k.trim().eq_ignore_ascii_case(key) {
                    *line = format!("{} = {}", key, value);
                    return;
                }
            }
        }
        self.lines.push(format!("{} = {}", key, value));
    }
}
