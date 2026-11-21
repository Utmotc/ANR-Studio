use std::fs;
use std::io;
use std::path::Path;

/// INI 文档：保留原文件的换行风格、UTF-8 BOM 与行内注释，写入用临时文件替换。
///
/// 0.3.0 起额外保证两件事：
/// 1. 只替换「值」，键名两侧的空格和行尾的 `; 注释` 原样保留；
/// 2. 原文件不是合法 UTF-8（例如 GBK 注释）时拒绝写回——lossy 解码会把用户的
///    字节变成不可逆的问号，比"写不进去"更糟。
pub struct IniDoc {
    pub lines: Vec<String>,
    nl: &'static str,
    bom: bool,
    lossy: bool,
}

impl IniDoc {
    pub fn load(path: &Path) -> Self {
        let bytes = fs::read(path).unwrap_or_default();
        let (bom, body, lossy) = decode(&bytes);
        let mut doc = Self::parse(&body);
        doc.bom = bom;
        doc.lossy = lossy;
        doc
    }

    /// 从文本构造（不碰文件系统）。`text` 里如果带 BOM 会被识别并保留。
    pub fn parse(text: &str) -> Self {
        let (bom, body) = match text.strip_prefix('\u{feff}') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let crlf = body.contains("\r\n");
        let nl = if crlf { "\r\n" } else { "\n" };
        let lines: Vec<String> = if crlf {
            body.split("\r\n").map(|s| s.to_string()).collect()
        } else {
            body.split('\n').map(|s| s.to_string()).collect()
        };
        Self {
            lines,
            nl,
            bom,
            lossy: false,
        }
    }

    /// 原文件不是合法 UTF-8：此时 `save` 会失败，界面应提示用户改用 UTF-8 重存。
    pub fn is_lossy(&self) -> bool {
        self.lossy
    }

    pub fn get(&self, section: &str, key: &str) -> Option<String> {
        let (head, tail) = self.section_range(section)?;
        for line in &self.lines[head + 1..tail] {
            let t = line.trim();
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

    /// 节区间 `[head, tail)`；`head` 是节标题行，`tail` 是下一个节标题行（或文件末尾）。
    fn section_range(&self, section: &str) -> Option<(usize, usize)> {
        let head = self
            .lines
            .iter()
            .position(|l| header_name(l).map(|n| n.eq_ignore_ascii_case(section)).unwrap_or(false))?;
        let tail = self
            .lines
            .iter()
            .enumerate()
            .skip(head + 1)
            .find(|(_, l)| header_name(l).is_some())
            .map(|(i, _)| i)
            .unwrap_or(self.lines.len());
        Some((head, tail))
    }

    pub fn set(&mut self, section: &str, key: &str, value: &str) {
        let Some((head, tail)) = self.section_range(section) else {
            // 没有这一节：追加到文件末尾（沿用原来的写法）
            if !self.lines.is_empty() && !self.lines.last().map(|l| l.trim().is_empty()).unwrap_or(true) {
                self.lines.push(String::new());
            }
            self.lines.push(format!("[{}]", section));
            self.lines.push(format!("{}={}", key, value));
            return;
        };

        for i in head + 1..tail {
            let line = &self.lines[i];
            let t = line.trim_start();
            if t.starts_with(';') || t.starts_with('#') {
                continue;
            }
            if let Some(eq) = line.find('=') {
                if line[..eq].trim().eq_ignore_ascii_case(key) {
                    let next = replace_value(line, value);
                    self.lines[i] = next;
                    return;
                }
            }
        }

        // 键不存在：插到本节最后一个非空行之后，不新增重复的节头
        let mut at = tail;
        while at > head + 1 && self.lines[at - 1].trim().is_empty() {
            at -= 1;
        }
        self.lines.insert(at, format!("{}={}", key, value));
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if self.lossy {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "配置文件不是 UTF-8 编码，为免写坏里面的注释已拒绝写入",
            ));
        }
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

    /// 扁平 ini（无 section）写键，保持行内注释与缩进
    pub fn set_flat(&mut self, key: &str, value: &str) {
        for line in self.lines.iter_mut() {
            let t = line.trim_start();
            if t.starts_with(';') || t.starts_with('#') {
                continue;
            }
            if let Some(eq) = line.find('=') {
                if line[..eq].trim().eq_ignore_ascii_case(key) {
                    *line = replace_value(line, value);
                    return;
                }
            }
        }
        self.lines.push(format!("{} = {}", key, value));
    }
}

/// 解码：返回 (BOM, 文本, 是否发生了 lossy 转换)
fn decode(bytes: &[u8]) -> (bool, String, bool) {
    let (bom, start) = if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF
    {
        (true, 3)
    } else {
        (false, 0)
    };
    let body = &bytes[start..];
    match std::str::from_utf8(body) {
        Ok(text) => (bom, text.to_string(), false),
        Err(_) => (bom, String::from_utf8_lossy(body).to_string(), true),
    }
}

fn header_name(line: &str) -> Option<&str> {
    let t = line.trim();
    let rest = t.strip_prefix('[')?;
    let end = rest.find(']')?;
    Some(rest[..end].trim())
}

/// 只替换 `=` 右边的值：保留缩进、键名、`=` 与空格、以及行尾的 `; 注释` / `# 注释`。
fn replace_value(line: &str, value: &str) -> String {
    let Some(eq) = line.find('=') else {
        return format!("{}={}", line, value);
    };
    let (head, rest) = line.split_at(eq + 1);
    let lead_len = rest.len() - rest.trim_start().len();
    let lead = &rest[..lead_len];

    // 值后面紧跟的注释：`;` / `#` 且前面是空白才算注释
    let mut comment_at = None;
    for (i, ch) in rest.char_indices() {
        if ch == ';' || ch == '#' {
            let before = rest[..i].chars().next_back();
            if before.map(|c| c.is_whitespace()).unwrap_or(false) {
                comment_at = Some(i);
                break;
            }
        }
    }
    let tail = match comment_at {
        Some(i) => format!(" {}", &rest[i..]),
        None => String::new(),
    };
    format!("{head}{lead}{value}{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_value_and_keeps_inline_comment() {
        let mut doc = IniDoc::parse("[FrameGen]\n  Enabled = true ; 关掉就没有插帧\nOther=1\n");
        doc.set("FrameGen", "Enabled", "false");
        assert_eq!(
            doc.lines[1], "  Enabled = false ; 关掉就没有插帧",
            "缩进、键名与行尾注释都应该保留"
        );
        assert_eq!(doc.lines[2], "Other=1");
    }

    #[test]
    fn keeps_crlf_and_bom() {
        let mut doc = IniDoc::parse("\u{feff}[A]\r\nx = 1\r\n");
        assert!(doc.bom);
        doc.set("A", "x", "2");
        let body = doc.lines.join(doc.nl);
        assert_eq!(body, "[A]\r\nx = 2\r\n");
    }

    #[test]
    fn writes_into_last_section_without_duplicating_header() {
        let mut doc = IniDoc::parse("[A]\nx=1\n\n[B]\ny=2\n");
        doc.set("B", "z", "3");
        let headers: Vec<&str> = doc
            .lines
            .iter()
            .filter(|l| header_name(l).is_some())
            .map(|l| l.trim())
            .collect();
        assert_eq!(headers, vec!["[A]", "[B]"], "不能出现重复的节头");
        assert_eq!(doc.get("B", "z").as_deref(), Some("3"));
    }

    #[test]
    fn missing_section_is_appended() {
        let mut doc = IniDoc::parse("[A]\nx=1\n");
        doc.set("FrameGen", "Enabled", "true");
        assert_eq!(doc.get("FrameGen", "Enabled").as_deref(), Some("true"));
        assert_eq!(header_name(doc.lines.last().unwrap()), None);
    }

    #[test]
    fn flat_set_keeps_comment() {
        let mut doc = IniDoc::parse("path = auto ; 卷积路径\nnits_per_unit=80\n");
        doc.set_flat("path", "dp4a");
        assert_eq!(doc.lines[0], "path = dp4a ; 卷积路径");
        assert_eq!(doc.lines[1], "nits_per_unit=80");
    }

    #[test]
    fn detects_non_utf8_and_refuses_to_save() {
        // GBK 编码的「中文」，不是合法 UTF-8
        let (bom, text, lossy) = decode(&[0xD6, 0xD0, 0xCE, 0xC4]);
        assert!(!bom);
        assert!(lossy, "非法 UTF-8 应该被识别出来");
        assert!(!text.is_empty());

        let mut doc = IniDoc::parse("[A]\nx=1\n");
        doc.lossy = true;
        // lossy 在读写磁盘之前就被拦下，所以这里不需要真实路径
        let err = doc.save(Path::new("Z:\\__no_such_dir__\\x.ini")).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn utf8_with_bom_is_not_lossy() {
        let (bom, text, lossy) = decode(&[0xEF, 0xBB, 0xBF, b'[', b'A', b']']);
        assert!(bom);
        assert!(!lossy);
        assert_eq!(text, "[A]");
    }
}