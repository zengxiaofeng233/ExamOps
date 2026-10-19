//! Durable single-document queue. The sidecar parallelizes individual PDF pages.
use super::{parse::{PageResult, ParsedPage}, DocumentTasks};
use crate::{database::AppPaths, error::{AppError, AppResult}};
use rusqlite::{params, Connection};
use tauri::{AppHandle, Manager};

pub fn enqueue(conn: &mut Connection, id: i64, keep_previous: bool) -> AppResult<()> {
    let tx = conn.transaction()?;
    let busy: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM document_jobs WHERE document_id=?1 AND state IN ('pending','running'))", [id], |r| r.get(0))?;
    if busy { return Err(AppError::invalid("该资料已在处理队列中")); }
    tx.execute("INSERT INTO document_jobs(document_id,keep_previous) VALUES(?1,?2)
        ON CONFLICT(document_id) DO UPDATE SET state='pending',keep_previous=excluded.keep_previous,error=NULL", params![id, keep_previous])?;
    tx.execute("UPDATE documents SET status='pending' WHERE id=?1", [id])?;
    tx.commit()?;
    Ok(())
}

pub fn begin(conn: &mut Connection, id: i64) -> AppResult<()> {
    let tx = conn.transaction()?;
    // Checkpoints are replayed and validated by the sidecar before becoming visible again.
    tx.execute("DELETE FROM document_page_drafts WHERE document_id=?1", [id])?;
    tx.execute("DELETE FROM document_images WHERE document_id=?1 AND EXISTS(SELECT 1 FROM documents WHERE id=?1 AND indexed_at IS NULL)", [id])?;
    tx.execute("UPDATE document_jobs SET state='running',completed_units=0,total_units=0,error=NULL WHERE document_id=?1", [id])?;
    super::mark_processing(&tx, id)?;
    tx.commit()?;
    Ok(())
}

pub fn store_page(conn: &mut Connection, id: i64, result: &PageResult) -> AppResult<()> {
    let tx = conn.transaction()?;
    tx.execute("INSERT INTO document_page_drafts(document_id,page_number,page_json) VALUES(?1,?2,?3)
        ON CONFLICT(document_id,page_number) DO UPDATE SET page_json=excluded.page_json",
        params![id, result.page.number, serde_json::to_string(&result.page)?])?;
    let first_import: bool = tx.query_row("SELECT indexed_at IS NULL FROM documents WHERE id=?1", [id], |r| r.get(0))?;
    if first_import {
        tx.execute("DELETE FROM document_images WHERE document_id=?1 AND page_number=?2", params![id,result.page.number])?;
        for image in &result.images {
            tx.execute("INSERT INTO document_images(document_id,page_number,ordinal,file_path,width,height,ocr_text) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![id,image.page,image.ordinal,image.path,image.width,image.height,image.ocr_text])?;
        }
    }
    tx.execute("UPDATE document_jobs SET completed_units=(SELECT COUNT(*) FROM document_page_drafts WHERE document_id=?1),
        total_units=MAX(total_units,?2,?3) WHERE document_id=?1", params![id,result.total,result.page.number])?;
    tx.commit()?;
    Ok(())
}

pub fn draft_pages(conn: &Connection, id: i64, start: i64, end: i64) -> AppResult<Vec<ParsedPage>> {
    let mut stmt = conn.prepare("SELECT page_json FROM document_page_drafts WHERE document_id=?1 AND page_number BETWEEN ?2 AND ?3 ORDER BY page_number")?;
    let rows = stmt.query_map(params![id,start,end], |r| r.get::<_, String>(0))?;
    rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
}

pub fn finish(conn: &Connection, id: i64, error: Option<&str>) -> AppResult<()> {
    // A successful final transaction already marks done. Do not overwrite a new
    // re-parse queued between that commit and this worker's cleanup.
    let changed = conn.execute("UPDATE document_jobs SET state=?2,error=?3 WHERE document_id=?1 AND state='running'", params![id, if error.is_some() { "failed" } else { "done" },error])?;
    if changed > 0 && error.is_none() {
        conn.execute("DELETE FROM document_page_drafts WHERE document_id=?1", [id])?;
    }
    Ok(())
}

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let tasks = app.state::<DocumentTasks>();
        let _worker = tasks.1.lock().await;
        let paths = app.state::<AppPaths>().inner().clone();
        loop {
            // Same lock as delete/edit: no document can disappear during a parse.
            let guard = tasks.0.lock().await;
            let next = (|| -> AppResult<Option<(i64,bool)>> {
                let mut conn = paths.connect()?;
                let mut stmt = conn.prepare("SELECT document_id,keep_previous FROM document_jobs WHERE state='pending' ORDER BY document_id LIMIT 1")?;
                let mut rows = stmt.query([])?;
                let next = rows.next()?.map(|r| Ok::<_,rusqlite::Error>((r.get(0)?,r.get(1)?))).transpose()?;
                drop(rows); drop(stmt);
                if let Some((id,_)) = next { begin(&mut conn, id)?; }
                Ok(next)
            })();
            let (id, previous) = match next { Ok(Some(job)) => job, _ => break };
            let result = super::process(&paths,id,previous, |_| {}).await;
            if let Ok(conn) = paths.connect() {
                let error = match &result {
                    Err(e) => Some(e.to_string()),
                    Ok(d) if d.status == "failed" => Some(d.error.clone().unwrap_or_else(|| "解析失败".into())),
                    _ => None,
                };
                if let Some(ref error) = error {
                    if previous { let _ = super::restore_after_failed_reparse(&conn,id); }
                    else { let _ = super::mark_failed(&conn,id,error); }
                }
                let _ = finish(&conn,id,error.as_deref());
            }
            drop(guard);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[tokio::test]
    #[ignore = "requires sidecar:build and EXAMOPS_TEST_PDF pointing to a multi-page OCR fixture"]
    async fn bundled_parser_resumes_after_interrupted_incremental_write() {
        let source = std::path::PathBuf::from(std::env::var("EXAMOPS_TEST_PDF").expect("EXAMOPS_TEST_PDF"));
        let (paths, mut conn) = test_support::course_with_document();
        let document = super::super::prepare(&mut conn,&paths.documents_dir,1,&source).unwrap();
        let id = document.id;
        crate::settings::save(&conn,crate::settings::AppSettings { ocr_gpu: false, ..Default::default() }).unwrap();
        enqueue(&mut conn,id,false).unwrap();
        begin(&mut conn,id).unwrap();
        let staging = super::super::figures_dir(&paths.documents_dir,1,id).with_extension("new");
        let cache = paths.documents_dir.join("1").join(format!("{id}_ocr"));
        let interrupted = super::super::parse::run_streaming(
            &std::path::PathBuf::from(&document.file_path),"pdf",&staging,&cache,"cpu", |_| {},
            |page| {
                store_page(&mut conn,id,&page).unwrap();
                if page.page.number == 2 { Err("simulate interruption".into()) } else { Ok(()) }
            }).await;
        assert!(interrupted.unwrap_err().contains("simulate interruption"));
        assert!(super::super::get(&conn,id).unwrap().completed_units >= 2);
        assert!(super::super::pages(&conn,id,1,2).unwrap().len() >= 2);
        assert!(crate::search::search_course(&conn,1,"English",Some(id),20).unwrap().hits.is_empty());
        drop(conn);
        crate::database::init_database(&paths).unwrap();
        let mut conn = paths.connect().unwrap();
        assert_eq!(super::super::get(&conn,id).unwrap().status,"pending");
        begin(&mut conn,id).unwrap();
        drop(conn);
        let finished = super::super::process(&paths,id,false, |_| {}).await.unwrap();
        assert_eq!(finished.status,"completed", "{:?}",finished.error);
        let conn = paths.connect().unwrap();
        assert_eq!(finished.page_count,4);
        assert!(!crate::search::search_course(&conn,1,"English",Some(id),20).unwrap().hits.is_empty());
        drop(conn); test_support::cleanup(paths);
    }

    #[test]
    fn drafts_are_durable_readable_and_not_searchable_until_commit() {
        let (paths, mut conn) = test_support::course_with_document();
        conn.execute("INSERT INTO documents(course_id,filename,file_type,file_path,file_size) VALUES(1,'scan.pdf','pdf','scan.pdf',1)", []).unwrap();
        let id = conn.last_insert_rowid();
        conn.execute("INSERT INTO document_contents(document_id) VALUES(?1)",[id]).unwrap();
        enqueue(&mut conn,id,false).unwrap();
        assert!(enqueue(&mut conn,id,false).is_err());
        begin(&mut conn,id).unwrap();
        let extraction = test_support::sample_extraction();
        let page = PageResult { page: extraction.pages[0].clone(), total: 3, images: vec![] };
        store_page(&mut conn,id,&page).unwrap();
        store_page(&mut conn,id,&page).unwrap(); // replay is idempotent
        let document = super::super::get(&conn,id).unwrap();
        assert_eq!((document.completed_units,document.total_units),(1,3));
        assert_eq!(super::super::pages(&conn,id,1,3).unwrap().len(),1);
        assert!(crate::search::search_course(&conn,1,"函数",Some(id),20).unwrap().hits.is_empty());
        drop(conn);
        crate::database::init_database(&paths).unwrap();
        let mut conn = paths.connect().unwrap();
        assert_eq!(super::super::get(&conn,id).unwrap().status,"pending");
        assert_eq!(draft_pages(&conn,id,1,3).unwrap().len(),1);
        begin(&mut conn,id).unwrap();
        store_page(&mut conn,id,&page).unwrap();
        super::super::apply_extraction(&mut conn,id,&extraction).unwrap();
        assert_eq!(super::super::get(&conn,id).unwrap().status,"completed");
        assert!(!crate::search::search_course(&conn,1,"函数",Some(id),20).unwrap().hits.is_empty());
        assert!(draft_pages(&conn,id,1,3).unwrap().is_empty());
        drop(conn); test_support::cleanup(paths);
    }

    #[test]
    fn interrupted_reparse_keeps_previous_pages_and_error() {
        let (paths, mut conn) = test_support::course_with_document();
        let before = super::super::body(&conn,1).unwrap();
        enqueue(&mut conn,1,true).unwrap();
        begin(&mut conn,1).unwrap();
        let mut page = test_support::sample_extraction().pages[0].clone();
        page.blocks[0].text = "unfinished replacement".into();
        store_page(&mut conn,1,&PageResult { page, total: 3, images: vec![] }).unwrap();
        assert_ne!(super::super::pages(&conn,1,1,1).unwrap()[0].blocks[0].text,"unfinished replacement");
        super::super::restore_after_failed_reparse(&conn,1).unwrap();
        finish(&conn,1,Some("GPU/CPU parse failed")).unwrap();
        assert_eq!(super::super::body(&conn,1).unwrap(),before);
        assert_eq!(super::super::get(&conn,1).unwrap().task_error.as_deref(),Some("GPU/CPU parse failed"));
        drop(conn); test_support::cleanup(paths);
    }

    #[test]
    fn finishing_old_job_does_not_discard_a_newly_queued_reparse() {
        let (paths, mut conn) = test_support::course_with_document();
        enqueue(&mut conn,1,true).unwrap();
        begin(&mut conn,1).unwrap();
        super::super::apply_extraction(&mut conn,1,&test_support::sample_extraction()).unwrap();
        enqueue(&mut conn,1,true).unwrap();
        finish(&conn,1,None).unwrap();
        let state: String = conn.query_row("SELECT state FROM document_jobs WHERE document_id=1",[],|r|r.get(0)).unwrap();
        assert_eq!(state,"pending");
        drop(conn); test_support::cleanup(paths);
    }
}
