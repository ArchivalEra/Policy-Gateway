#![allow(dead_code)]
//! redb — 本地持久化存储
//!
//! 全局单例，通过 init_store() 初始化，然后通过 store() 访问。

use redb::{Database, TableDefinition, ReadableTable};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

const TABLE: TableDefinition<&str, &str> = TableDefinition::new("auth");
static GLOBAL_STORE: OnceLock<Arc<Mutex<Database>>> = OnceLock::new();

/// 初始化全局数据库（启动时调用一次）
pub fn init_store(path: &str) -> Result<(), String> {
    // 确保父目录存在
    if let Some(parent) = Path::new(path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {}", e))?;
    }
    let db = Database::open(Path::new(path)).map_err(|e| format!("DB open: {}", e))?;
    let write_txn = db.begin_write().map_err(|e| format!("DB begin: {}", e))?;
    let _ = write_txn.open_table(TABLE);
    write_txn.commit().map_err(|e| format!("DB commit: {}", e))?;
    GLOBAL_STORE.set(Arc::new(Mutex::new(db)))
        .map_err(|_| "DB already initialized".to_string())
}

fn store() -> Option<&'static Arc<Mutex<Database>>> {
    GLOBAL_STORE.get()
}

/// 写入一个条目
pub async fn put(sha256: &str, value: &str) -> Result<(), String> {
    let store = match store() {
        Some(s) => s.clone(),
        None => return Ok(()),  // store not initialized, skip
    };
    let db = store.lock().await;
    let tx = db.begin_write().map_err(|e| format!("write begin: {}", e))?;
    {
        let mut table = tx.open_table(TABLE).map_err(|e| format!("table open: {}", e))?;
        table.insert(sha256, value).map_err(|e| format!("insert: {}", e))?;
    }
    tx.commit().map_err(|e| format!("commit: {}", e))?;
    Ok(())
}

/// 读取一个条目
pub async fn get(sha256: &str) -> Result<Option<String>, String> {
    let store = match store() {
        Some(s) => s.clone(),
        None => return Ok(None),
    };
    let db = store.lock().await;
    let tx = db.begin_read().map_err(|e| format!("read begin: {}", e))?;
    let table = tx.open_table(TABLE).map_err(|e| format!("table open: {}", e))?;
    match table.get(sha256).map_err(|e| format!("get: {}", e))? {
        Some(v) => Ok(Some(v.value().to_string())),
        None => Ok(None),
    }
}

/// 删除一个条目
pub async fn delete(sha256: &str) -> Result<(), String> {
    let store = match store() {
        Some(s) => s.clone(),
        None => return Ok(()),
    };
    let db = store.lock().await;
    let tx = db.begin_write().map_err(|e| format!("write begin: {}", e))?;
    {
        let mut table = tx.open_table(TABLE).map_err(|e| format!("table open: {}", e))?;
        table.remove(sha256).map_err(|e| format!("remove: {}", e))?;
    }
    tx.commit().map_err(|e| format!("commit: {}", e))?;
    Ok(())
}

/// 列出所有条目（迭代器）
pub async fn iter() -> Result<Vec<(String, String)>, String> {
    let store = match store() {
        Some(s) => s.clone(),
        None => return Ok(vec![]),
    };
    let db = store.lock().await;
    let tx = db.begin_read().map_err(|e| format!("read begin: {}", e))?;
    let table = tx.open_table(TABLE).map_err(|e| format!("table open: {}", e))?;
    let mut out = Vec::new();
    for result in table.iter().map_err(|e| format!("iter: {}", e))? {
        let (k, v) = result.map_err(|e| format!("entry: {}", e))?;
        out.push((k.value().to_string(), v.value().to_string()));
    }
    Ok(out)
}
