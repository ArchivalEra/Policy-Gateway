#![allow(dead_code)]
//! redb — 本地持久化存储
//!
//! 将权限表持久化到本地文件，重启不丢失。
//! 只有安装了 worker-mirror 模块才可省略此文件。

use redb::{Database, TableDefinition, ReadableTable};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

const TABLE: TableDefinition<&str, &str> = TableDefinition::new("auth");

pub struct Store {
    db: Arc<Mutex<Database>>,
}

impl Store {
    /// 打开或创建数据库
    pub fn open(path: &str) -> Result<Self, String> {
        let db = Database::open(Path::new(path)).map_err(|e| format!("DB open: {}", e))?;
        // 创建表（如果不存在）
        let write_txn = db.begin_write().map_err(|e| format!("DB begin: {}", e))?;
        let _ = write_txn.open_table(TABLE);
        write_txn.commit().map_err(|e| format!("DB commit: {}", e))?;
        Ok(Self { db: Arc::new(Mutex::new(db)) })
    }

    /// 写入一个条目
    pub async fn put(&self, sha256: &str, value: &str) -> Result<(), String> {
        let db = self.db.lock().await;
        let tx = db.begin_write().map_err(|e| format!("write begin: {}", e))?;
        {
            let mut table = tx.open_table(TABLE).map_err(|e| format!("table open: {}", e))?;
            table.insert(sha256, value).map_err(|e| format!("insert: {}", e))?;
        }
        tx.commit().map_err(|e| format!("commit: {}", e))?;
        Ok(())
    }

    /// 读取一个条目
    pub async fn get(&self, sha256: &str) -> Result<Option<String>, String> {
        let db = self.db.lock().await;
        let tx = db.begin_read().map_err(|e| format!("read begin: {}", e))?;
        let table = tx.open_table(TABLE).map_err(|e| format!("table open: {}", e))?;
        match table.get(sha256).map_err(|e| format!("get: {}", e))? {
            Some(v) => Ok(Some(v.value().to_string())),
            None => Ok(None),
        }
    }

    /// 删除一个条目
    pub async fn delete(&self, sha256: &str) -> Result<(), String> {
        let db = self.db.lock().await;
        let tx = db.begin_write().map_err(|e| format!("write begin: {}", e))?;
        {
            let mut table = tx.open_table(TABLE).map_err(|e| format!("table open: {}", e))?;
            table.remove(sha256).map_err(|e| format!("remove: {}", e))?;
        }
        tx.commit().map_err(|e| format!("commit: {}", e))?;
        Ok(())
    }

    /// 列出所有条目（迭代器）
    pub async fn iter(&self) -> Result<Vec<(String, String)>, String> {
        let db = self.db.lock().await;
        let tx = db.begin_read().map_err(|e| format!("read begin: {}", e))?;
        let table = tx.open_table(TABLE).map_err(|e| format!("table open: {}", e))?;
        let mut out = Vec::new();
        for result in table.iter().map_err(|e| format!("iter: {}", e))? {
            let (k, v) = result.map_err(|e| format!("entry: {}", e))?;
            out.push((k.value().to_string(), v.value().to_string()));
        }
        Ok(out)
    }
}
