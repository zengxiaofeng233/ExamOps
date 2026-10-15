//! Relocation is performed before opening the UI, after the previous process exited.
use crate::database::{self, AppPaths};
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const CONFIG: &str = "storage-location.json";
const ROOTS: [&str; 3] = ["examops.db", "documents", "cache"];

#[derive(Default, Serialize, Deserialize)]
struct Location {
    active: Option<PathBuf>,
    pending: Option<PathBuf>,
    #[serde(default)]
    previous: Vec<Previous>,
}

#[derive(Serialize, Deserialize)]
struct Previous {
    directory: PathBuf,
    files: Vec<Stamp>,
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
struct Stamp {
    path: PathBuf,
    bytes: u64,
    digest: String,
}

#[derive(Serialize)]
pub struct StorageInfo {
    pub pending_dir: Option<PathBuf>,
    pub previous_dirs: Vec<PathBuf>,
}

pub struct StorageState {
    default_dir: PathBuf,
    // Retained for the process lifetime, including migration and cleanup.
    _locks: Vec<File>,
    operation: Mutex<()>,
}

fn load(default_dir: &Path) -> AppResult<Location> {
    match fs::read(default_dir.join(CONFIG)) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Location::default()),
        Err(e) => Err(e.into()),
    }
}

fn save(default_dir: &Path, location: &Location) -> AppResult<()> {
    let pending = default_dir.join("storage-location.json.tmp");
    let mut file = File::create(&pending)?;
    file.write_all(&serde_json::to_vec_pretty(location)?)?;
    file.sync_all()?;
    drop(file);
    fs::rename(pending, default_dir.join(CONFIG))?;
    Ok(())
}

fn lock(directory: &Path) -> AppResult<File> {
    let file = OpenOptions::new().read(true).write(true).create(true).truncate(false)
        .open(directory.join(".examops-storage.lock"))?;
    file.try_lock().map_err(|_| AppError::msg("数据目录正在被另一个 ExamOps 进程使用，请关闭它后重试"))?;
    Ok(file)
}

fn reject_link(path: &Path) -> AppResult<()> {
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(AppError::msg("数据迁移不支持符号链接或目录联接，请选择普通本机文件夹"));
        }
    }
    if metadata.file_type().is_symlink() {
        return Err(AppError::msg("数据迁移不支持符号链接"));
    }
    Ok(())
}

fn check_root(path: &Path) -> AppResult<PathBuf> {
    if !path.is_absolute() || !path.is_dir() {
        return Err(AppError::msg("请选择已存在的本机文件夹"));
    }
    for ancestor in path.ancestors() {
        reject_link(ancestor)?;
    }
    #[cfg(windows)]
    if !matches!(path.components().next(), Some(std::path::Component::Prefix(p))
        if matches!(p.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_))) {
        return Err(AppError::msg("请选择本机磁盘文件夹，不支持网络或设备路径"));
    }
    Ok(fs::canonicalize(path)?)
}

fn stamp(root: &Path, path: &Path) -> AppResult<Stamp> {
    let mut file = File::open(path)?;
    let mut context = md5::Context::new();
    let mut buffer = [0; 65536];
    let mut bytes = 0;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 { break; }
        context.consume(&buffer[..count]);
        bytes += count as u64;
    }
    Ok(Stamp { path: path.strip_prefix(root).map_err(|_| AppError::msg("数据路径越界"))?.into(), bytes, digest: format!("{:x}", context.compute()) })
}

fn collect(root: &Path, path: &Path, files: &mut Vec<Stamp>) -> AppResult<()> {
    reject_link(path)?;
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            collect(root, &entry?.path(), files)?;
        }
    } else if path.is_file() {
        files.push(stamp(root, path)?);
    } else {
        return Err(AppError::msg("数据目录包含不支持的文件类型"));
    }
    Ok(())
}

fn inventory(root: &Path) -> AppResult<Vec<Stamp>> {
    let mut files = Vec::new();
    for name in ROOTS {
        let path = root.join(name);
        if path.try_exists()? { collect(root, &path, &mut files)?; }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

#[cfg(windows)]
fn check_space(directory: &Path, required: u64) -> AppResult<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(path: *const u16, available: *mut u64, total: *mut u64, free: *mut u64) -> i32;
    }
    let path: Vec<u16> = directory.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut available = 0;
    // The null-terminated path and output pointer remain valid for this call.
    if unsafe { GetDiskFreeSpaceExW(path.as_ptr(), &mut available, std::ptr::null_mut(), std::ptr::null_mut()) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    if available < required.saturating_add(16 * 1024 * 1024) {
        return Err(AppError::msg("目标磁盘空间不足，请至少预留数据大小加 16 MB 的空间"));
    }
    Ok(())
}

#[cfg(not(windows))]
fn check_space(_directory: &Path, _required: u64) -> AppResult<()> { Ok(()) }

fn target(source: &Path, requested: &Path) -> AppResult<PathBuf> {
    let destination = check_root(requested)?;
    let source = fs::canonicalize(source)?;
    if destination.starts_with(&source) || source.starts_with(&destination) {
        return Err(AppError::msg("新旧数据目录不能相同，也不能互相包含"));
    }
    if fs::read_dir(&destination)?.next().is_some() {
        return Err(AppError::msg("请选择空文件夹，避免覆盖已有文件"));
    }
    Ok(destination)
}

fn verify_database(path: &Path) -> AppResult<()> {
    let conn = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let result: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if result != "ok" { return Err(AppError::msg("数据库完整性检查失败，保留原数据")); }
    Ok(())
}

fn relocate(source: &Path, destination: &Path) -> AppResult<Previous> {
    let destination = target(source, destination)?;
    let conn = database::open_connection(&source.join("examops.db"))?;
    let busy: i64 = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))?;
    if busy != 0 { return Err(AppError::msg("数据库仍在使用，请关闭其他 ExamOps 窗口")); }
    // Hold a write lock through copy/verification so an older app cannot change the database.
    conn.execute_batch("BEGIN IMMEDIATE")?;
    verify_database(&source.join("examops.db"))?;
    let files = inventory(source)?;
    check_space(&destination, files.iter().map(|f| f.bytes).sum())?;
    for entry in &files {
        let output = destination.join(&entry.path);
        if let Some(parent) = output.parent() { fs::create_dir_all(parent)?; }
        let mut input = File::open(source.join(&entry.path))?;
        let mut output = OpenOptions::new().write(true).create_new(true).open(output)?;
        std::io::copy(&mut input, &mut output)?;
        output.sync_all()?;
    }
    if inventory(&destination)? != files || inventory(source)? != files {
        return Err(AppError::msg("文件校验失败，保留原数据；请另选空文件夹重试"));
    }
    // Stored absolute paths must point at the new copies even while the old copies exist.
    let mut copied = database::open_connection(&destination.join("examops.db"))?;
    let tx = copied.transaction()?;
    for table in ["documents", "document_images"] {
        let rows: Vec<(i64, String)> = {
            let mut stmt = tx.prepare(&format!("SELECT id, file_path FROM {table}"))?;
            let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        for (id, old) in rows {
            let old = Path::new(&old);
            // Canonicalize existing files to handle Windows casing and extended path prefixes.
            let normalized = fs::canonicalize(old).unwrap_or_else(|_| old.to_path_buf());
            if let Ok(relative) = normalized.strip_prefix(fs::canonicalize(source)?) {
                tx.execute(&format!("UPDATE {table} SET file_path = ?1 WHERE id = ?2"),
                    rusqlite::params![destination.join(relative).to_string_lossy(), id])?;
            }
        }
    }
    tx.commit()?;
    drop(copied);
    verify_database(&destination.join("examops.db"))?;
    Ok(Previous { directory: source.into(), files })
}

impl StorageState {
    pub fn start(default_dir: PathBuf) -> AppResult<(Self, AppPaths)> {
        fs::create_dir_all(&default_dir)?;
        let mut locks = vec![lock(&default_dir)?];
        let mut location = load(&default_dir)?;
        let mut source = location.active.clone().unwrap_or_else(|| default_dir.clone());
        if location.active.is_some() && !source.join("examops.db").is_file() {
            return Err(AppError::msg(format!("数据目录不可用：{}。请连接原磁盘或恢复该目录；不会自动创建空数据库。", source.display())));
        }
        check_root(&source)?;
        if fs::canonicalize(&source)? != fs::canonicalize(&default_dir)? { locks.push(lock(&source)?); }
        if let Some(destination) = location.pending.take() {
            match relocate(&source, &destination) {
                Ok(previous) => {
                    let destination_lock = lock(&destination)?;
                    location.previous.push(previous);
                    location.active = Some(destination.clone());
                    save(&default_dir, &location)?;
                    locks.truncate(1);
                    locks.push(destination_lock);
                    source = destination;
                }
                Err(error) => {
                    save(&default_dir, &location)?;
                    return Err(AppError::msg(format!("迁移失败：{error}\n原数据仍在 {}。关闭本提示后重新打开软件即可使用原目录；目标中的未完成副本请手动处理。", source.display())));
                }
            }
        }
        Ok((Self { default_dir, _locks: locks, operation: Mutex::new(()) }, AppPaths::new(source)))
    }

    pub fn info(&self) -> AppResult<StorageInfo> {
        let _guard = self.operation.lock().map_err(|_| AppError::msg("存储操作失败"))?;
        let location = load(&self.default_dir)?;
        Ok(StorageInfo { pending_dir: location.pending, previous_dirs: location.previous.into_iter().map(|p| p.directory).collect() })
    }

    pub fn schedule(&self, paths: &AppPaths, directory: Option<PathBuf>) -> AppResult<()> {
        let _guard = self.operation.lock().map_err(|_| AppError::msg("存储操作失败"))?;
        let mut location = load(&self.default_dir)?;
        location.pending = match directory {
            Some(directory) => {
                let destination = target(&paths.data_dir, &directory)?;
                // A write probe checks permissions without consuming space on the source disk.
                let probe = destination.join(".examops-write-test");
                let file = OpenOptions::new().write(true).create_new(true).open(&probe)?;
                drop(file);
                fs::remove_file(probe)?;
                Some(destination)
            }
            None => None,
        };
        save(&self.default_dir, &location)
    }

    pub fn cleanup(&self, paths: &AppPaths, directory: PathBuf) -> AppResult<()> {
        let _guard = self.operation.lock().map_err(|_| AppError::msg("存储操作失败"))?;
        let mut location = load(&self.default_dir)?;
        let index = location.previous.iter().position(|p| p.directory == directory)
            .ok_or_else(|| AppError::msg("该目录不是已迁移的旧数据目录"))?;
        let previous = &location.previous[index];
        let old = fs::canonicalize(&directory)?;
        let current = fs::canonicalize(&paths.data_dir)?;
        if current.starts_with(&old) || old.starts_with(&current) { return Err(AppError::msg("不能清理当前数据目录")); }
        // Never traverse a replaced junction or delete data changed since the migration.
        for ancestor in directory.ancestors() { reject_link(ancestor)?; }
        let _old_lock = if old != fs::canonicalize(&self.default_dir)? { Some(lock(&directory)?) } else { None };
        verify_database(&paths.db_path)?;
        // Windows refuses to delete an open SQLite file. Keep the legacy writer check
        // separate from deletion; current app instances are excluded by the file lock.
        if directory.join("examops.db").exists() {
            let old_db = database::open_connection(&directory.join("examops.db"))?;
            old_db.execute_batch("BEGIN IMMEDIATE; ROLLBACK;")?;
        }
        for suffix in ["examops.db-wal", "examops.db-journal"] {
            if fs::metadata(directory.join(suffix)).is_ok_and(|m| m.len() > 0) {
                return Err(AppError::msg("旧目录存在新的数据库写入，已停止清理，请手动检查"));
            }
        }
        let remaining = inventory(&directory)?;
        if remaining.iter().any(|entry| !previous.files.contains(entry)) {
            return Err(AppError::msg("旧数据已发生变化，已停止清理，请手动检查"));
        }
        for entry in remaining { fs::remove_file(directory.join(entry.path))?; }
        // Only remove now-empty application directories, never the entire selected folder.
        fn empty_dirs(path: &Path) {
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    if entry.file_type().is_ok_and(|kind| kind.is_dir()) { empty_dirs(&entry.path()); }
                }
            }
            let _ = fs::remove_dir(path);
        }
        for name in ["documents", "cache"] { empty_dirs(&directory.join(name)); }
        location.previous.remove(index);
        save(&self.default_dir, &location)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{documents, test_support};

    fn destination(source: &Path, tag: &str) -> PathBuf {
        let name = format!("{}-{tag}", source.file_name().unwrap().to_string_lossy());
        let path = if let Some(root) = std::env::var_os("EXAMOPS_STORAGE_TEST_ROOT") {
            PathBuf::from(root).join(name)
        } else {
            source.with_file_name(name)
        };
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn restart_migrates_records_files_and_paths_then_cleanup_preserves_other_files() {
        let (original, conn) = test_support::course_with_document();
        let db_source = original.data_dir.clone();
        let document = documents::get(&conn, 1).unwrap();
        let copy = documents::locate_copy(&original.documents_dir, &document);
        let source_bytes = fs::read(&copy).unwrap();
        let figures = documents::figures_dir(&original.documents_dir, 1, 1);
        fs::create_dir_all(&figures).unwrap();
        fs::write(figures.join("one.png"), b"figure").unwrap();
        conn.execute("INSERT INTO document_images (document_id, page_number, ordinal, file_path, width, height) VALUES (1, 1, 1, ?1, 10, 10)",
            [figures.join("one.png").to_string_lossy()]).unwrap();
        conn.execute("INSERT INTO notes (course_id, title, content) VALUES (1, '迁移测试', '保留笔记与公式')", []).unwrap();
        crate::settings::set_value(&conn, "migration_test", &"保留设置").unwrap();
        fs::write(original.cache_dir.join("preview.png"), b"preview").unwrap();
        fs::write(original.data_dir.join("unrelated.txt"), b"keep").unwrap();
        drop(conn);
        let dest = destination(&db_source, "destination");
        eprintln!("storage relocation test: {} -> {}", db_source.display(), dest.display());
        let (state, paths) = StorageState::start(db_source.clone()).unwrap();
        state.schedule(&paths, Some(dest.clone())).unwrap();
        // The active directory does not change until the old process is gone.
        assert_eq!(paths.data_dir, db_source);
        assert!(state.info().unwrap().pending_dir.is_some());
        drop(state);
        let (state, moved) = StorageState::start(db_source.clone()).unwrap();
        assert_eq!(fs::canonicalize(&moved.data_dir).unwrap(), fs::canonicalize(&dest).unwrap());
        let conn = moved.connect().unwrap();
        let doc = documents::get(&conn, 1).unwrap();
        assert!(Path::new(&doc.file_path).starts_with(&moved.documents_dir));
        assert_eq!(fs::read(documents::locate_copy(&moved.documents_dir, &doc)).unwrap(), source_bytes);
        assert!(copy.exists(), "old copy must remain before confirmation");
        assert_eq!(fs::read(moved.cache_dir.join("preview.png")).unwrap(), b"preview");
        assert_eq!(fs::read(documents::figures_dir(&moved.documents_dir, 1, 1).join("one.png")).unwrap(), b"figure");
        assert!(!documents::body(&conn, 1).unwrap().is_empty());
        assert!(documents::image_path(&conn, &moved.documents_dir, 1).unwrap().starts_with(&moved.documents_dir));
        let note: String = conn.query_row("SELECT content FROM notes WHERE title = '迁移测试'", [], |r| r.get(0)).unwrap();
        assert_eq!(note, "保留笔记与公式");
        assert_eq!(crate::settings::get_value::<String>(&conn, "migration_test").unwrap().as_deref(), Some("保留设置"));
        drop(conn);
        state.cleanup(&moved, db_source.clone()).unwrap();
        assert!(!original.db_path.exists());
        assert!(!copy.exists());
        assert!(db_source.join("unrelated.txt").exists());
        assert!(db_source.join(CONFIG).exists());
        assert!(moved.db_path.exists());
        assert!(state.info().unwrap().previous_dirs.is_empty());
        drop(state);
        let (state, reopened) = StorageState::start(db_source).unwrap();
        assert_eq!(reopened.data_dir, moved.data_dir);
        drop(state);
        test_support::cleanup(original);
        test_support::cleanup(moved);
    }

    #[test]
    fn invalid_destinations_and_cancel_do_not_change_current_data() {
        let original = test_support::temp_paths("storage-validation");
        let (state, paths) = StorageState::start(original.data_dir.clone()).unwrap();
        assert!(StorageState::start(original.data_dir.clone()).is_err());
        assert!(state.schedule(&paths, Some(original.data_dir.clone())).is_err());
        assert!(state.schedule(&paths, Some(original.cache_dir.clone())).is_err());
        assert!(state.schedule(&paths, Some(original.data_dir.parent().unwrap().into())).is_err());
        let dest = destination(&original.data_dir, "validation");
        fs::write(dest.join("existing.txt"), b"keep").unwrap();
        assert!(state.schedule(&paths, Some(dest.clone())).is_err());
        fs::remove_file(dest.join("existing.txt")).unwrap();
        state.schedule(&paths, Some(dest.clone())).unwrap();
        state.schedule(&paths, None).unwrap();
        assert!(state.info().unwrap().pending_dir.is_none());
        drop(state);
        fs::remove_dir(&dest).unwrap();
        test_support::cleanup(original);
    }

    #[test]
    fn failed_startup_migration_keeps_original_and_cancels_pending() {
        let original = test_support::temp_paths("storage-failure");
        let dest = destination(&original.data_dir, "failure");
        let (state, paths) = StorageState::start(original.data_dir.clone()).unwrap();
        state.schedule(&paths, Some(dest.clone())).unwrap();
        drop(state);
        fs::write(dest.join("other.txt"), b"do not overwrite").unwrap();
        assert!(StorageState::start(original.data_dir.clone()).is_err());
        let (state, paths) = StorageState::start(original.data_dir.clone()).unwrap();
        assert_eq!(paths.data_dir, original.data_dir);
        assert!(state.info().unwrap().pending_dir.is_none());
        assert!(original.db_path.exists());
        assert_eq!(fs::read(dest.join("other.txt")).unwrap(), b"do not overwrite");
        drop(state);
        fs::remove_file(dest.join("other.txt")).unwrap();
        fs::remove_dir(dest).unwrap();
        test_support::cleanup(original);
    }

    #[test]
    fn modified_old_data_is_not_deleted_and_missing_active_dir_never_creates_empty_db() {
        let original = test_support::temp_paths("storage-cleanup");
        let dest = destination(&original.data_dir, "cleanup");
        let (state, paths) = StorageState::start(original.data_dir.clone()).unwrap();
        state.schedule(&paths, Some(dest.clone())).unwrap();
        drop(state);
        let (state, moved) = StorageState::start(original.data_dir.clone()).unwrap();
        fs::write(original.documents_dir.join("new.txt"), b"new data").unwrap();
        assert!(state.cleanup(&moved, original.data_dir.clone()).is_err());
        assert!(original.db_path.exists());
        assert!(original.documents_dir.join("new.txt").exists());
        assert!(state.cleanup(&moved, moved.data_dir.clone()).is_err());
        drop(state);
        fs::rename(moved.db_path.clone(), moved.db_path.with_extension("backup")).unwrap();
        assert!(StorageState::start(original.data_dir.clone()).is_err());
        assert!(!moved.db_path.exists());
        test_support::cleanup(original);
        test_support::cleanup(moved);
    }

    #[test]
    fn a_second_migration_can_clean_the_previous_custom_directory() {
        let original = test_support::temp_paths("storage-repeat");
        let first = destination(&original.data_dir, "first");
        let second = destination(&original.data_dir, "second");
        let (state, paths) = StorageState::start(original.data_dir.clone()).unwrap();
        state.schedule(&paths, Some(first)).unwrap();
        drop(state);
        let (state, first) = StorageState::start(original.data_dir.clone()).unwrap();
        state.schedule(&first, Some(second)).unwrap();
        drop(state);
        let (state, second) = StorageState::start(original.data_dir.clone()).unwrap();
        state.cleanup(&second, first.data_dir.clone()).unwrap();
        assert!(!first.db_path.exists());
        assert!(second.db_path.exists());
        drop(state);
        test_support::cleanup(original);
        test_support::cleanup(first);
        test_support::cleanup(second);
    }

    #[cfg(windows)]
    #[test]
    fn insufficient_space_is_reported_without_copying() {
        let original = test_support::temp_paths("storage-space");
        assert!(check_space(&original.data_dir, u64::MAX).is_err());
        test_support::cleanup(original);
    }
}
