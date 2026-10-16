//! Running the Python document processor and decoding its structured output
//! (protocol v2, see `python-sidecar/main.py` and docs/API.md).

use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ParsedBlock {
    pub text: String,
    #[serde(default)]
    pub heading: Option<u8>,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub raw_text: Option<String>,
    #[serde(default)]
    pub bbox: Option<[f64; 4]>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub recognition: String,
    #[serde(default)]
    pub issues: Vec<String>,
    #[serde(default)]
    pub formula: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPage {
    pub number: i64,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub width: Option<f64>,
    #[serde(default)]
    pub height: Option<f64>,
    #[serde(default)]
    pub ocr: bool,
    #[serde(default)]
    pub loc_start: i64,
    #[serde(default)]
    pub loc_end: i64,
    #[serde(default)]
    pub blocks: Vec<ParsedBlock>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ParsedImage {
    pub page: i64,
    pub ordinal: i64,
    pub path: String,
    pub width: i64,
    pub height: i64,
    #[serde(default)]
    pub ocr_text: String,
}

#[derive(Debug, Deserialize)]
struct ParserResponse {
    status: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    loc_kind: Option<String>,
    #[serde(default)]
    structure_source: Option<String>,
    #[serde(default)]
    pages: Vec<ParsedPage>,
    #[serde(default)]
    images: Vec<ParsedImage>,
    #[serde(default)]
    warnings: Vec<String>,
}

/// Everything the app stores about one processed document.
#[derive(Debug, Clone)]
pub struct Extraction {
    pub content: String,
    pub loc_kind: String,
    pub structure_source: String,
    pub pages: Vec<ParsedPage>,
    pub images: Vec<ParsedImage>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseProgress {
    #[serde(default)]
    pub stage: String,
    pub current: u64,
    pub total: u64,
}

#[derive(Deserialize)]
struct ProgressLine {
    progress: ParseProgress,
}

fn loc_kind_for(file_type: &str) -> &'static str {
    match file_type {
        "pdf" => "page",
        "pptx" => "slide",
        "docx" => "paragraph",
        _ => "line",
    }
}

/// Rebuilds units from phase-one plain text ("[第 N 页]" markers) or older parsers.
pub fn pages_from_plain_text(content: &str, file_type: &str) -> Vec<ParsedPage> {
    let marker = regex::Regex::new(r"^\[第 (\d+) (?:页|张幻灯片)\]$").expect("valid regex");
    let mut pages: Vec<ParsedPage> = Vec::new();
    let blocks_of = |text: &str| -> Vec<ParsedBlock> {
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| ParsedBlock { text: l.trim().to_string(), heading: None, kind: "text".into(), ..Default::default() })
            .collect()
    };
    match file_type {
        "pdf" | "pptx" => {
            let unit = if file_type == "pdf" { "页" } else { "张幻灯片" };
            let mut current: Option<(i64, String)> = None;
            for line in content.lines() {
                if let Some(captures) = marker.captures(line.trim()) {
                    if let Some((number, text)) = current.take() {
                        pages.push(page(number, format!("第 {number} {unit}"), number, number, blocks_of(&text)));
                    }
                    current = Some((captures[1].parse().unwrap_or(pages.len() as i64 + 1), String::new()));
                } else {
                    let entry = current.get_or_insert((1, String::new()));
                    entry.1.push_str(line);
                    entry.1.push('\n');
                }
            }
            if let Some((number, text)) = current {
                pages.push(page(number, format!("第 {number} {unit}"), number, number, blocks_of(&text)));
            }
        }
        "docx" => {
            for (index, line) in content.lines().filter(|l| !l.trim().is_empty()).enumerate() {
                let n = index as i64 + 1;
                pages.push(page(n, format!("第 {n} 段"), n, n, blocks_of(line)));
            }
        }
        _ => {
            let mut start = 0usize;
            let mut buffer: Vec<&str> = Vec::new();
            let lines: Vec<&str> = content.lines().collect();
            for (index, line) in lines.iter().enumerate() {
                if line.trim().is_empty() || buffer.len() >= 40 {
                    if !buffer.is_empty() {
                        let n = pages.len() as i64 + 1;
                        let (a, b) = (start as i64 + 1, (start + buffer.len()) as i64);
                        let label = if a == b { format!("第 {a} 行") } else { format!("第 {a}–{b} 行") };
                        pages.push(page(n, label, a, b, vec![ParsedBlock {
                            text: buffer.join("\n"), heading: None, kind: "text".into(), ..Default::default()
                        }]));
                        buffer.clear();
                    }
                    if line.trim().is_empty() {
                        start = index + 1;
                        continue;
                    }
                    start = index;
                }
                buffer.push(line);
            }
            if !buffer.is_empty() {
                let n = pages.len() as i64 + 1;
                let (a, b) = (start as i64 + 1, (start + buffer.len()) as i64);
                let label = if a == b { format!("第 {a} 行") } else { format!("第 {a}–{b} 行") };
                pages.push(page(n, label, a, b, vec![ParsedBlock { text: buffer.join("\n"), heading: None, kind: "text".into(), ..Default::default() }]));
            }
        }
    }
    pages.retain(|p| !p.blocks.is_empty());
    pages
}

fn page(number: i64, label: String, loc_start: i64, loc_end: i64, blocks: Vec<ParsedBlock>) -> ParsedPage {
    ParsedPage { number, label, width: None, height: None, ocr: false, loc_start, loc_end, blocks }
}

fn decode(stdout: &[u8], file_type: &str) -> Result<Extraction, String> {
    let response: ParserResponse = serde_json::from_slice(stdout)
        .map_err(|e| format!("文档处理程序返回无效结果：{e}"))?;
    if response.status != "success" {
        return Err(response.message.unwrap_or_else(|| "正文提取失败".into()));
    }
    let content = response.content.unwrap_or_default();
    if content.trim().is_empty() {
        return Err("未提取到正文".into());
    }
    let mut pages = response.pages;
    let mut structure_source = response.structure_source.unwrap_or_default();
    if pages.is_empty() {
        // Older processor without structure: derive units from the text.
        pages = pages_from_plain_text(&content, file_type);
        structure_source = "legacy".into();
    }
    for page in &mut pages {
        if page.loc_start == 0 {
            page.loc_start = page.number;
        }
        if page.loc_end == 0 {
            page.loc_end = page.loc_start;
        }
    }
    Ok(Extraction {
        content,
        loc_kind: response.loc_kind.unwrap_or_else(|| loc_kind_for(file_type).to_string()),
        structure_source,
        pages,
        images: response.images,
        warnings: response.warnings,
    })
}

/// Runs the processor on `source`, saving figures into `figures_dir`.
/// `on_progress` receives page/slide progress while the file is processed.
pub async fn run(
    source: &Path,
    file_type: &str,
    figures_dir: &Path,
    mut on_progress: impl FnMut(ParseProgress) + Send,
) -> Result<Extraction, String> {
    let mut command = crate::sidecar::command()?;
    command
        .arg("--input")
        .arg(source)
        .arg("--figures-dir")
        .arg(figures_dir)
        .arg("--progress")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| format!("无法启动文档处理程序：{e}"))?;
    let mut stdout = child.stdout.take().ok_or("无法读取文档处理程序输出")?;
    let stderr = child.stderr.take().ok_or("无法读取文档处理程序输出")?;
    let reader = tokio::spawn(async move {
        let mut buffer = Vec::new();
        stdout.read_to_end(&mut buffer).await.map(|_| buffer)
    });
    let mut lines = BufReader::new(stderr).lines();
    let mut diagnostics: Vec<String> = Vec::new();
    while let Ok(Some(line)) = lines.next_line().await {
        match serde_json::from_str::<ProgressLine>(&line) {
            Ok(ProgressLine { progress }) => on_progress(progress),
            Err(_) => {
                if diagnostics.len() >= 20 {
                    diagnostics.remove(0);
                }
                diagnostics.push(line);
            }
        }
    }
    let status = child.wait().await.map_err(|e| e.to_string())?;
    let stdout = reader
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("文档处理程序退出异常：{}", diagnostics.join("\n")));
    }
    decode(&stdout, file_type)
}

/// Renders PDF pages to PNG files in `out_dir` (one-shot processor call).
pub async fn render_pages(pdf: &Path, pages: &[i64], out_dir: &Path, width: u32) -> Result<(), String> {
    let mut command = crate::sidecar::command()?;
    let list = pages.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",");
    command
        .arg("--render")
        .arg(pdf)
        .arg("--pages")
        .arg(list)
        .arg("--out-dir")
        .arg(out_dir)
        .arg("--width")
        .arg(width.to_string());
    let output = command
        .output()
        .await
        .map_err(|e| format!("无法启动文档处理程序：{e}"))?;
    #[derive(Deserialize)]
    struct RenderResponse {
        status: String,
        #[serde(default)]
        message: Option<String>,
    }
    let response: RenderResponse = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("页面预览生成失败：{e}"))?;
    if response.status == "success" {
        Ok(())
    } else {
        Err(response.message.unwrap_or_else(|| "页面预览生成失败".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_pdf_text_becomes_pages() {
        let pages = pages_from_plain_text("[第 1 页]\n标题\n正文\n\n[第 3 页]\n第三页", "pdf");
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].label, "第 1 页");
        assert_eq!(pages[0].blocks.len(), 2);
        assert_eq!(pages[1].number, 3);
    }

    #[test]
    fn legacy_text_becomes_line_blocks() {
        let pages = pages_from_plain_text("一\n二\n\n三", "txt");
        assert_eq!(pages.iter().map(|p| p.label.as_str()).collect::<Vec<_>>(), vec!["第 1–2 行", "第 4 行"]);
        assert_eq!((pages[1].loc_start, pages[1].loc_end), (4, 4));
    }

    #[test]
    fn analysis_fields_survive_protocol_round_trip() {
        let json = r#"{"text":"𝑥²≠α","kind":"ocr","raw_text":"𝑥²≠α","bbox":[1,2,30,40],"source":"patterns","recognition":"suspect","issues":["U+E000"],"formula":true}"#;
        let block: ParsedBlock = serde_json::from_str(json).unwrap();
        assert_eq!(block.raw_text.as_deref(), Some("𝑥²≠α"));
        assert_eq!(block.bbox, Some([1., 2., 30., 40.]));
        assert!(block.formula);
        assert_eq!(serde_json::from_str::<ParsedBlock>(&serde_json::to_string(&block).unwrap()).unwrap(), block);
        let legacy: ParsedBlock = serde_json::from_str(r#"{"text":"旧正文"}"#).unwrap();
        assert!(legacy.bbox.is_none() && legacy.issues.is_empty());
    }

    #[test]
    fn decode_reports_parser_errors_and_fills_locations() {
        assert_eq!(
            decode(r#"{"status":"error","message":"PDF 已加密"}"#.as_bytes(), "pdf").unwrap_err(),
            "PDF 已加密"
        );
        let extraction = decode(
            r#"{"status":"success","content":"x","loc_kind":"page","pages":[{"number":2,"blocks":[{"text":"x"}]}]}"#.as_bytes(),
            "pdf",
        )
        .unwrap();
        assert_eq!((extraction.pages[0].loc_start, extraction.pages[0].loc_end), (2, 2));
        let legacy = decode(r#"{"status":"success","content":"正文"}"#.as_bytes(), "txt").unwrap();
        assert_eq!(legacy.structure_source, "legacy");
        assert_eq!(legacy.loc_kind, "line");
    }
}
