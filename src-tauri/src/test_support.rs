//! Helpers shared by unit tests: temporary data directories and sample courses.

use crate::database::AppPaths;
use crate::documents::{self, parse::{Extraction, ParsedBlock, ParsedPage}};
use rusqlite::Connection;

pub fn temp_paths(tag: &str) -> AppPaths {
    let folder = std::env::temp_dir().join(format!(
        "examops-{tag}-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    ));
    let paths = AppPaths::new(folder);
    crate::database::init_database(&paths).unwrap();
    paths
}

pub fn cleanup(paths: AppPaths) {
    let _ = std::fs::remove_dir_all(paths.data_dir);
}

fn block(text: &str, heading: Option<u8>) -> ParsedBlock {
    ParsedBlock { text: text.into(), heading, kind: "text".into() }
}

/// A three-page calculus excerpt with two chapters and one sub-section.
pub fn sample_extraction() -> Extraction {
    let page = |n: i64, blocks| ParsedPage {
        number: n,
        label: format!("第 {n} 页"),
        width: Some(595.0),
        height: Some(842.0),
        ocr: false,
        loc_start: n,
        loc_end: n,
        blocks,
    };
    Extraction {
        content: "第一章 函数与极限\n函数的定义……".into(),
        loc_kind: "page".into(),
        structure_source: "patterns".into(),
        pages: vec![
            page(1, vec![block("第一章 函数与极限", Some(1)), block("函数是两个非空数集之间的对应关系。", None)]),
            page(2, vec![block("1.1 极限的定义", Some(2)), block("数列极限描述了数列的变化趋势，导数建立在极限之上。", None)]),
            page(3, vec![block("第二章 导数", Some(1)), block("导数描述函数在某一点的变化率，导数的几何意义是切线斜率。", None)]),
        ],
        images: vec![],
        warnings: vec![],
    }
}

/// Course 1 with an indexed sample document (id 1); course 2 is empty.
pub fn course_with_document() -> (AppPaths, Connection) {
    let paths = temp_paths("course");
    let mut conn = paths.connect().unwrap();
    conn.execute("INSERT INTO courses (name) VALUES ('高等数学'), ('线性代数')", []).unwrap();
    let source = paths.data_dir.join("高等数学讲义.pdf");
    std::fs::write(&source, "%PDF-1.4").unwrap();
    let document = documents::prepare(&mut conn, &paths.documents_dir, 1, &source).unwrap();
    documents::apply_extraction(&mut conn, document.id, &sample_extraction()).unwrap();
    (paths, conn)
}
