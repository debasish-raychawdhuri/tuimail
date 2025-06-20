use crate::email::{Email, EmailAttachment, EmailAddress};
use anyhow::{Result, Context};
use chrono::{DateTime, Local, TimeZone};
use rusqlite::{Connection, params};
use serde_json;
use std::path::Path;

pub struct EmailDatabase {
    conn: Connection,
    db_path: std::path::PathBuf,
}

impl EmailDatabase {
    pub fn new(db_path: &Path) -> Result<Self> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create database directory: {:?}", parent))?;
        }

        let conn = Connection::open(db_path)
            .with_context(|| format!("Failed to open database: {:?}", db_path))?;

        let db = EmailDatabase { 
            conn,
            db_path: db_path.to_path_buf(),
        };
        db.initialize_schema()?;
        Ok(db)
    }

    pub fn get_database_path(&self) -> String {
        self.db_path.to_string_lossy().to_string()
    }

    fn initialize_schema(&self) -> Result<()> {
        // Create emails table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS emails (
                uid INTEGER NOT NULL,
                account_email TEXT NOT NULL,
                folder TEXT NOT NULL,
                message_id TEXT,
                subject TEXT NOT NULL,
                from_addresses TEXT NOT NULL, -- JSON array
                to_addresses TEXT NOT NULL,   -- JSON array
                cc_addresses TEXT,            -- JSON array
                bcc_addresses TEXT,           -- JSON array
                date_received INTEGER NOT NULL, -- Unix timestamp
                body_text TEXT,
                body_html TEXT,
                flags TEXT NOT NULL,          -- JSON array
                headers TEXT NOT NULL,        -- JSON object
                seen BOOLEAN NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                PRIMARY KEY(account_email, folder, uid)
            )",
            [],
        )?;

        // Create attachments table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS attachments (
                id INTEGER PRIMARY KEY,
                account_email TEXT NOT NULL,
                folder TEXT NOT NULL,
                email_uid INTEGER NOT NULL,
                filename TEXT NOT NULL,
                content_type TEXT NOT NULL,
                data BLOB NOT NULL,
                size INTEGER NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                FOREIGN KEY(account_email, folder, email_uid) REFERENCES emails(account_email, folder, uid) ON DELETE CASCADE
            )",
            [],
        )?;

        // Create folder metadata table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS folder_metadata (
                id INTEGER PRIMARY KEY,
                account_email TEXT NOT NULL,
                folder TEXT NOT NULL,
                last_uid INTEGER NOT NULL DEFAULT 0,
                total_messages INTEGER NOT NULL DEFAULT 0,
                last_sync INTEGER NOT NULL DEFAULT 0, -- Unix timestamp
                UNIQUE(account_email, folder)
            )",
            [],
        )?;

        // Create indexes for better performance
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_emails_account_folder 
             ON emails(account_email, folder)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_emails_uid 
             ON emails(account_email, folder, uid)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_emails_date 
             ON emails(account_email, folder, date_received DESC)",
            [],
        )?;

        // Simple index on timestamp for efficient MAX() queries
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_emails_timestamp 
             ON emails(date_received DESC)",
            [],
        )?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_attachments_email 
             ON attachments(account_email, folder, email_uid)",
            [],
        )?;

        Ok(())
    }

    pub fn save_emails(&self, account_email: &str, folder: &str, emails: &[Email]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        for email in emails {
            // Parse UID from email.id (which is stored as string)
            let uid: u32 = email.id.parse().unwrap_or(0);
            
            // Insert or replace email
            tx.execute(
                "INSERT OR REPLACE INTO emails (
                    uid, account_email, folder, message_id, subject,
                    from_addresses, to_addresses, cc_addresses, bcc_addresses,
                    date_received, body_text, body_html, flags, headers, seen
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    uid,
                    account_email,
                    folder,
                    None::<String>, // We don't have message_id in current Email struct
                    email.subject,
                    serde_json::to_string(&email.from)?,
                    serde_json::to_string(&email.to)?,
                    serde_json::to_string(&email.cc)?,
                    serde_json::to_string(&email.bcc)?,
                    email.date.timestamp(),
                    email.body_text.as_deref(),
                    email.body_html.as_deref(),
                    serde_json::to_string(&email.flags)?,
                    serde_json::to_string(&email.headers)?,
                    email.seen,
                ],
            )?;

            // Delete existing attachments for this email
            tx.execute(
                "DELETE FROM attachments WHERE account_email = ?1 AND folder = ?2 AND email_uid = ?3",
                params![account_email, folder, uid],
            )?;

            // Insert attachments
            for attachment in &email.attachments {
                tx.execute(
                    "INSERT INTO attachments (account_email, folder, email_uid, filename, content_type, data, size)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        account_email,
                        folder,
                        uid,
                        attachment.filename,
                        attachment.content_type,
                        attachment.data,
                        attachment.data.len() as i64,
                    ],
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn load_emails(&self, account_email: &str, folder: &str) -> Result<Vec<Email>> {
        let mut stmt = self.conn.prepare(
            "SELECT uid, message_id, subject, from_addresses, to_addresses, 
                    cc_addresses, bcc_addresses, date_received, body_text, body_html,
                    flags, headers, seen
             FROM emails 
             WHERE account_email = ?1 AND folder = ?2 
             ORDER BY date_received DESC",
        )?;

        let email_rows = stmt.query_map(params![account_email, folder], |row| {
            Ok((
                row.get::<_, u32>(0)?,       // uid
                row.get::<_, Option<String>>(1)?, // message_id
                row.get::<_, String>(2)?,    // subject
                row.get::<_, String>(3)?,    // from_addresses
                row.get::<_, String>(4)?,    // to_addresses
                row.get::<_, String>(5)?,    // cc_addresses
                row.get::<_, String>(6)?,    // bcc_addresses
                row.get::<_, i64>(7)?,       // date_received
                row.get::<_, Option<String>>(8)?, // body_text
                row.get::<_, Option<String>>(9)?, // body_html
                row.get::<_, String>(10)?,   // flags
                row.get::<_, String>(11)?,   // headers
                row.get::<_, bool>(12)?,     // seen
            ))
        })?;

        let mut emails = Vec::new();
        for row_result in email_rows {
            let (uid, _message_id, subject, from_json, to_json, cc_json, bcc_json,
                 date_timestamp, body_text, body_html, flags_json, headers_json, seen) = row_result?;

            // Load attachments for this email
            let mut attachment_stmt = self.conn.prepare(
                "SELECT filename, content_type, data FROM attachments 
                 WHERE account_email = ?1 AND folder = ?2 AND email_uid = ?3"
            )?;
            
            let attachment_rows = attachment_stmt.query_map(params![account_email, folder, uid], |row| {
                Ok(EmailAttachment {
                    filename: row.get(0)?,
                    content_type: row.get(1)?,
                    data: row.get(2)?,
                })
            })?;

            let mut attachments = Vec::new();
            for attachment_result in attachment_rows {
                attachments.push(attachment_result?);
            }

            let email = Email {
                id: uid.to_string(), // Convert UID back to string for Email struct
                subject,
                from: serde_json::from_str(&from_json)?,
                to: serde_json::from_str(&to_json)?,
                cc: serde_json::from_str(&cc_json)?,
                bcc: serde_json::from_str(&bcc_json)?,
                date: DateTime::from_timestamp(date_timestamp, 0)
                    .unwrap_or_else(|| Local::now().into())
                    .with_timezone(&Local),
                body_text,
                body_html,
                attachments,
                flags: serde_json::from_str(&flags_json)?,
                headers: serde_json::from_str(&headers_json)?,
                seen,
                folder: folder.to_string(),
            };

            emails.push(email);
        }

        Ok(emails)
    }

    pub fn save_folder_metadata(&self, account_email: &str, folder: &str, last_uid: u32, total_messages: u32) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO folder_metadata (account_email, folder, last_uid, total_messages, last_sync)
             VALUES (?1, ?2, ?3, ?4, strftime('%s', 'now'))",
            params![account_email, folder, last_uid, total_messages],
        )?;
        Ok(())
    }

    pub fn load_folder_metadata(&self, account_email: &str, folder: &str) -> Result<(u32, u32, i64)> {
        let result = self.conn.query_row(
            "SELECT last_uid, total_messages, last_sync FROM folder_metadata 
             WHERE account_email = ?1 AND folder = ?2",
            params![account_email, folder],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        );

        match result {
            Ok((last_uid, total_messages, last_sync)) => Ok((last_uid, total_messages, last_sync)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok((0, 0, 0)),
            Err(e) => Err(e.into()),
        }
    }

    #[allow(dead_code)]
    pub fn get_email_count(&self, account_email: &str, folder: &str) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM emails WHERE account_email = ?1 AND folder = ?2",
            params![account_email, folder],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    #[allow(dead_code)]
    pub fn delete_emails_by_folder(&self, account_email: &str, folder: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM emails WHERE account_email = ?1 AND folder = ?2",
            params![account_email, folder],
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn vacuum(&self) -> Result<()> {
        self.conn.execute("VACUUM", [])?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_database_size(&self) -> Result<u64> {
        let size: i64 = self.conn.query_row(
            "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
            [],
            |row| row.get(0),
        )?;
        Ok(size as u64)
    }

    // Sync daemon specific methods
    pub fn execute_sql(&self, sql: &str) -> Result<()> {
        self.conn.execute(sql, [])?;
        Ok(())
    }

    pub fn save_sync_state(&self, account_email: &str, folder: &str, last_uid: u32, last_sync: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO sync_state (account_email, folder, last_uid_seen, last_sync_timestamp, sync_in_progress)
             VALUES (?1, ?2, ?3, ?4, FALSE)",
            params![account_email, folder, last_uid, last_sync],
        )?;
        Ok(())
    }

    pub fn get_sync_state(&self, account_email: &str, folder: &str) -> Result<(u32, i64, bool)> {
        let result = self.conn.query_row(
            "SELECT last_uid_seen, last_sync_timestamp, sync_in_progress FROM sync_state 
             WHERE account_email = ?1 AND folder = ?2",
            params![account_email, folder],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        );

        match result {
            Ok((last_uid, last_sync, in_progress)) => Ok((last_uid, last_sync, in_progress)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok((0, 0, false)),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_sync_in_progress(&self, account_email: &str, folder: &str, in_progress: bool) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO sync_state (account_email, folder, sync_in_progress, last_uid_seen, last_sync_timestamp)
             VALUES (?1, ?2, ?3, 
                     COALESCE((SELECT last_uid_seen FROM sync_state WHERE account_email = ?1 AND folder = ?2), 0),
                     COALESCE((SELECT last_sync_timestamp FROM sync_state WHERE account_email = ?1 AND folder = ?2), 0))",
            params![account_email, folder, in_progress],
        )?;
        Ok(())
    }

    pub fn queue_email_operation(&self, account_email: &str, operation_type: &str, 
                                email_uid: u32, folder: &str, target_folder: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO email_operations (account_email, operation_type, email_uid, folder, target_folder, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, strftime('%s', 'now'))",
            params![account_email, operation_type, email_uid, folder, target_folder],
        )?;
        Ok(())
    }

    pub fn get_pending_operations(&self) -> Result<Vec<(i64, String, String, u32, String, Option<String>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, account_email, operation_type, email_uid, folder, target_folder
             FROM email_operations 
             WHERE processed = FALSE 
             ORDER BY created_at ASC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, u32>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?;

        let mut operations = Vec::new();
        for row in rows {
            operations.push(row?);
        }

        Ok(operations)
    }

    pub fn mark_operation_processed(&self, operation_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE email_operations SET processed = TRUE WHERE id = ?1",
            params![operation_id],
        )?;
        Ok(())
    }

    pub fn mark_operation_failed(&self, operation_id: i64, error: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE email_operations SET error = ?1 WHERE id = ?2",
            params![error, operation_id],
        )?;
        Ok(())
    }

    pub fn get_emails_paginated(&self, account_email: &str, folder: &str, 
                               offset: usize, limit: usize) -> Result<Vec<Email>> {
        let mut stmt = self.conn.prepare(
            "SELECT uid, message_id, subject, from_addresses, to_addresses, 
                    cc_addresses, bcc_addresses, date_received, body_text, body_html,
                    flags, headers, seen
             FROM emails 
             WHERE account_email = ?1 AND folder = ?2 
             ORDER BY date_received DESC
             LIMIT ?3 OFFSET ?4",
        )?;

        let email_rows = stmt.query_map(params![account_email, folder, limit, offset], |row| {
            Ok((
                row.get::<_, u32>(0)?,       // uid
                row.get::<_, Option<String>>(1)?, // message_id
                row.get::<_, String>(2)?,    // subject
                row.get::<_, String>(3)?,    // from_addresses
                row.get::<_, String>(4)?,    // to_addresses
                row.get::<_, String>(5)?,    // cc_addresses
                row.get::<_, String>(6)?,    // bcc_addresses
                row.get::<_, i64>(7)?,       // date_received
                row.get::<_, Option<String>>(8)?, // body_text
                row.get::<_, Option<String>>(9)?, // body_html
                row.get::<_, String>(10)?,   // flags
                row.get::<_, String>(11)?,   // headers
                row.get::<_, bool>(12)?,     // seen
            ))
        })?;

        let mut emails = Vec::new();
        for row_result in email_rows {
            let (uid, _message_id, subject, from_json, to_json, cc_json, bcc_json,
                 date_timestamp, body_text, body_html, flags_json, headers_json, seen) = row_result?;

            // Load attachments for this email
            let mut attachment_stmt = self.conn.prepare(
                "SELECT filename, content_type, data FROM attachments 
                 WHERE account_email = ?1 AND folder = ?2 AND email_uid = ?3"
            )?;
            
            let attachment_rows = attachment_stmt.query_map(params![account_email, folder, uid], |row| {
                Ok(crate::email::EmailAttachment {
                    filename: row.get(0)?,
                    content_type: row.get(1)?,
                    data: row.get(2)?,
                })
            })?;

            let mut attachments = Vec::new();
            for attachment_result in attachment_rows {
                attachments.push(attachment_result?);
            }

            let email = Email {
                id: uid.to_string(), // Convert UID back to string for Email struct
                subject,
                from: serde_json::from_str(&from_json)?,
                to: serde_json::from_str(&to_json)?,
                cc: serde_json::from_str(&cc_json)?,
                bcc: serde_json::from_str(&bcc_json)?,
                date: chrono::DateTime::from_timestamp(date_timestamp, 0)
                    .unwrap_or_else(|| chrono::Local::now().into())
                    .with_timezone(&chrono::Local),
                body_text,
                body_html,
                attachments,
                flags: serde_json::from_str(&flags_json)?,
                headers: serde_json::from_str(&headers_json)?,
                seen,
                folder: folder.to_string(),
            };

            emails.push(email);
        }

        Ok(emails)
    }

    pub fn get_all_emails(&self, account_email: &str, folder: &str) -> Result<Vec<Email>> {
        let mut stmt = self.conn.prepare(
            "SELECT uid, message_id, subject, from_addresses, to_addresses, 
                    cc_addresses, bcc_addresses, date_received, body_text, body_html,
                    flags, headers, seen
             FROM emails 
             WHERE account_email = ?1 AND folder = ?2 
             ORDER BY date_received DESC",
        )?;

        let email_rows = stmt.query_map(params![account_email, folder], |row| {
            Ok((
                row.get::<_, u32>(0)?,       // uid
                row.get::<_, Option<String>>(1)?, // message_id
                row.get::<_, String>(2)?,    // subject
                row.get::<_, String>(3)?,    // from_addresses
                row.get::<_, String>(4)?,    // to_addresses
                row.get::<_, String>(5)?,    // cc_addresses
                row.get::<_, String>(6)?,    // bcc_addresses
                row.get::<_, i64>(7)?,       // date_received
                row.get::<_, Option<String>>(8)?, // body_text
                row.get::<_, Option<String>>(9)?, // body_html
                row.get::<_, String>(10)?,   // flags
                row.get::<_, String>(11)?,   // headers
                row.get::<_, bool>(12)?,     // seen
            ))
        })?;

        let mut emails = Vec::new();
        for row_result in email_rows {
            let (uid, _message_id, subject, from_json, to_json, cc_json, bcc_json,
                 date_timestamp, body_text, body_html, flags_json, headers_json, seen) = row_result?;

            // Load attachments for this email
            let mut attachment_stmt = self.conn.prepare(
                "SELECT filename, content_type, data FROM attachments 
                 WHERE account_email = ?1 AND folder = ?2 AND email_uid = ?3"
            )?;
            
            let attachment_rows = attachment_stmt.query_map(params![account_email, folder, uid], |row| {
                Ok(crate::email::EmailAttachment {
                    filename: row.get(0)?,
                    content_type: row.get(1)?,
                    data: row.get(2)?,
                })
            })?;

            let mut attachments = Vec::new();
            for attachment_result in attachment_rows {
                attachments.push(attachment_result?);
            }

            let email = Email {
                id: uid.to_string(), // Convert UID back to string for Email struct
                subject,
                from: serde_json::from_str(&from_json)?,
                to: serde_json::from_str(&to_json)?,
                cc: serde_json::from_str(&cc_json)?,
                bcc: serde_json::from_str(&bcc_json)?,
                date: chrono::DateTime::from_timestamp(date_timestamp, 0)
                    .unwrap_or_else(|| chrono::Local::now().into())
                    .with_timezone(&chrono::Local),
                body_text,
                body_html,
                attachments,
                flags: serde_json::from_str(&flags_json)?,
                headers: serde_json::from_str(&headers_json)?,
                seen,
                folder: folder.to_string(),
            };

            emails.push(email);
        }

        Ok(emails)
    }

    /// Get recent emails with a limit for better performance
    /// Get the timestamp of the most recent email - much faster than loading emails
    pub fn get_latest_email_timestamp_old(&self, account_email: &str, folder: &str) -> Result<Option<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT MAX(date_received) FROM emails WHERE account_email = ?1 AND folder = ?2"
        )?;
        
        let timestamp = stmt.query_row(params![account_email, folder], |row| {
            row.get::<_, Option<i64>>(0)
        })?;
        
        Ok(timestamp)
    }

    pub fn get_recent_emails(&self, account_email: &str, folder: &str, limit: usize) -> Result<Vec<Email>> {
        let mut stmt = self.conn.prepare(
            "SELECT uid, message_id, subject, from_addresses, to_addresses, 
                    cc_addresses, bcc_addresses, date_received, body_text, body_html,
                    flags, headers, seen
             FROM emails 
             WHERE account_email = ?1 AND folder = ?2 
             ORDER BY date_received DESC
             LIMIT ?3",
        )?;

        let email_rows = stmt.query_map(params![account_email, folder, limit], |row| {
            Ok((
                row.get::<_, u32>(0)?,       // uid
                row.get::<_, Option<String>>(1)?, // message_id
                row.get::<_, String>(2)?,    // subject
                row.get::<_, String>(3)?,    // from_addresses
                row.get::<_, String>(4)?,    // to_addresses
                row.get::<_, String>(5)?,    // cc_addresses
                row.get::<_, String>(6)?,    // bcc_addresses
                row.get::<_, i64>(7)?,       // date_received
                row.get::<_, Option<String>>(8)?, // body_text
                row.get::<_, Option<String>>(9)?, // body_html
                row.get::<_, String>(10)?,   // flags
                row.get::<_, String>(11)?,   // headers
                row.get::<_, bool>(12)?,     // seen
            ))
        })?;

        // First, collect all email UIDs and basic data
        let mut email_data = Vec::new();
        for row_result in email_rows {
            let (uid, _message_id, subject, from_str, to_str, cc_str, bcc_str, date_received, 
                 body_text, body_html, flags_str, headers_str, seen) = row_result?;
            email_data.push((uid, subject, from_str, to_str, cc_str, bcc_str, date_received, 
                           body_text, body_html, flags_str, headers_str, seen));
        }
        
        // Load ALL attachments for these emails in one query (much faster!)
        let uids: Vec<String> = email_data.iter().map(|(uid, ..)| uid.to_string()).collect();
        let uid_placeholders = uids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        
        let attachment_query = format!(
            "SELECT email_uid, filename, content_type, data FROM attachments 
             WHERE account_email = ? AND folder = ? AND email_uid IN ({})",
            uid_placeholders
        );
        
        let mut attachment_stmt = self.conn.prepare(&attachment_query)?;
        let mut params = vec![account_email.to_string(), folder.to_string()];
        params.extend(uids);
        
        let attachment_rows = attachment_stmt.query_map(
            rusqlite::params_from_iter(params.iter()),
            |row| {
                Ok((
                    row.get::<_, u32>(0)?,  // email_uid
                    crate::email::EmailAttachment {
                        filename: row.get(1)?,
                        content_type: row.get(2)?,
                        data: row.get(3)?,
                    }
                ))
            }
        )?;

        // Group attachments by email UID
        let mut attachments_by_uid: std::collections::HashMap<u32, Vec<crate::email::EmailAttachment>> = 
            std::collections::HashMap::new();
        
        for attachment_result in attachment_rows {
            let (email_uid, attachment) = attachment_result?;
            attachments_by_uid.entry(email_uid).or_insert_with(Vec::new).push(attachment);
        }
        
        // Now build the final email objects
        let mut emails = Vec::new();
        
        for (uid, subject, from_str, to_str, cc_str, bcc_str, date_received, 
             body_text, body_html, flags_str, headers_str, seen) in email_data {
            
            // Parse addresses
            let from_addresses: Vec<crate::email::EmailAddress> = serde_json::from_str(&from_str).unwrap_or_default();
            let to_addresses: Vec<crate::email::EmailAddress> = serde_json::from_str(&to_str).unwrap_or_default();
            let cc_addresses: Vec<crate::email::EmailAddress> = serde_json::from_str(&cc_str).unwrap_or_default();
            let bcc_addresses: Vec<crate::email::EmailAddress> = serde_json::from_str(&bcc_str).unwrap_or_default();

            // Parse flags
            let flags: Vec<String> = serde_json::from_str(&flags_str).unwrap_or_default();

            // Parse headers
            let headers: std::collections::HashMap<String, String> = 
                serde_json::from_str(&headers_str).unwrap_or_default();

            // Get attachments for this email (already loaded)
            let attachments = attachments_by_uid.remove(&uid).unwrap_or_default();

            let email = Email {
                id: uid.to_string(),
                subject,
                from: from_addresses,
                to: to_addresses,
                cc: cc_addresses,
                bcc: bcc_addresses,
                date: DateTime::from_timestamp(date_received, 0)
                    .unwrap_or_else(|| DateTime::from_timestamp(0, 0).unwrap())
                    .with_timezone(&Local),
                body_text,
                body_html,
                attachments,
                flags,
                headers,
                seen,
                folder: folder.to_string(),
            };

            emails.push(email);
        }

        Ok(emails)
    }

    pub fn update_email_seen_status(&self, account_email: &str, folder: &str, uid: u32, seen: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE emails SET seen = ?1, updated_at = strftime('%s', 'now') 
             WHERE account_email = ?2 AND folder = ?3 AND uid = ?4",
            params![seen, account_email, folder, uid],
        )?;
        Ok(())
    }

    pub fn is_sync_stale(&self, account_email: &str, folder: &str, max_age_seconds: i64) -> Result<bool> {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;

        let last_sync = self.conn.query_row(
            "SELECT last_sync_timestamp FROM sync_state 
             WHERE account_email = ?1 AND folder = ?2",
            params![account_email, folder],
            |row| row.get::<_, i64>(0),
        ).unwrap_or(0);

        Ok(current_time - last_sync > max_age_seconds)
    }

    pub fn clear_folder_emails(&self, account_email: &str, folder: &str) -> Result<()> {
        // Clear emails for this folder
        self.conn.execute(
            "DELETE FROM emails WHERE account_email = ?1 AND folder = ?2",
            params![account_email, folder],
        )?;
        
        // Clear attachments for this folder
        self.conn.execute(
            "DELETE FROM attachments WHERE account_email = ?1 AND folder = ?2",
            params![account_email, folder],
        )?;
        
        // Reset folder metadata
        self.conn.execute(
            "DELETE FROM folder_metadata WHERE account_email = ?1 AND folder = ?2",
            params![account_email, folder],
        )?;
        
        // Reset sync state
        self.conn.execute(
            "DELETE FROM sync_state WHERE account_email = ?1 AND folder = ?2",
            params![account_email, folder],
        )?;
        
        Ok(())
    }
    
    /// Get the highest UID for a specific account and folder (for new mail checking)
    pub fn get_last_uid(&self, account_email: &str, folder: &str) -> Result<u32> {
        let result = self.conn.query_row(
            "SELECT MAX(CAST(uid AS INTEGER)) FROM emails WHERE account_email = ?1 AND folder = ?2",
            params![account_email, folder],
            |row| {
                let uid: Option<i64> = row.get(0)?;
                Ok(uid.unwrap_or(0) as u32)
            }
        );
        
        match result {
            Ok(uid) => Ok(uid),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
            Err(e) => Err(e.into()),
        }
    }

    /// Get all account/folder combinations in the database
    pub fn get_all_folders(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT account_email, folder FROM emails ORDER BY account_email, folder"
        )?;
        
        let folder_iter = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
            ))
        })?;
        
        let mut folders = Vec::new();
        for folder in folder_iter {
            folders.push(folder?);
        }
        
        Ok(folders)
    }

    /// Get the latest email timestamp for an account/folder
    pub fn get_latest_email_timestamp(&self, account_email: &str, folder: &str) -> Result<chrono::DateTime<chrono::Utc>> {
        let result = self.conn.query_row(
            "SELECT MAX(date) FROM emails WHERE account_email = ?1 AND folder = ?2",
            params![account_email, folder],
            |row| {
                let date_str: Option<String> = row.get(0)?;
                Ok(date_str)
            }
        );
        
        match result {
            Ok(Some(date_str)) => {
                // Parse the date string to DateTime<Utc>
                if let Ok(local_dt) = chrono::DateTime::parse_from_rfc3339(&date_str) {
                    Ok(local_dt.with_timezone(&chrono::Utc))
                } else if let Ok(local_dt) = chrono::DateTime::parse_from_str(&date_str, "%Y-%m-%d %H:%M:%S%.f %z") {
                    Ok(local_dt.with_timezone(&chrono::Utc))
                } else {
                    // Fallback to current time if parsing fails
                    Ok(chrono::Utc::now())
                }
            }
            Ok(None) | Err(rusqlite::Error::QueryReturnedNoRows) => {
                // No emails found, return epoch
                Ok(chrono::DateTime::from_timestamp(0, 0).unwrap_or_else(chrono::Utc::now))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Get emails that arrived after a specific timestamp
    pub fn get_emails_since_timestamp(
        &self, 
        account_email: &str, 
        folder: &str, 
        since: chrono::DateTime<chrono::Utc>
    ) -> Result<Vec<Email>> {
        let since_timestamp = since.timestamp();
        
        let mut stmt = self.conn.prepare(
            "SELECT uid, message_id, subject, from_addresses, to_addresses, cc_addresses, bcc_addresses, 
             date_received, body_text, body_html, flags, headers_json, seen
             FROM emails 
             WHERE account_email = ?1 AND folder = ?2 AND date_received > ?3
             ORDER BY date_received DESC"
        )?;
        
        let email_data: Result<Vec<_>, _> = stmt.query_map(params![account_email, folder, since_timestamp], |row| {
            Ok((
                row.get::<_, u32>(0)?,      // uid
                row.get::<_, String>(1)?,   // message_id
                row.get::<_, String>(2)?,   // subject
                row.get::<_, String>(3)?,   // from_addresses
                row.get::<_, String>(4)?,   // to_addresses
                row.get::<_, String>(5)?,   // cc_addresses
                row.get::<_, String>(6)?,   // bcc_addresses
                row.get::<_, i64>(7)?,      // date_received
                row.get::<_, Option<String>>(8)?, // body_text
                row.get::<_, Option<String>>(9)?, // body_html
                row.get::<_, String>(10)?,  // flags
                row.get::<_, String>(11)?,  // headers_json
                row.get::<_, bool>(12)?,    // seen
            ))
        })?.collect();
        
        let email_data = email_data?;
        
        if email_data.is_empty() {
            return Ok(Vec::new());
        }
        
        // Load ALL attachments for these emails in one query (much faster!)
        let uids: Vec<String> = email_data.iter().map(|(uid, ..)| uid.to_string()).collect();
        let uid_placeholders = uids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        
        let attachment_query = format!(
            "SELECT email_uid, filename, content_type, data FROM attachments 
             WHERE account_email = ? AND folder = ? AND email_uid IN ({})",
            uid_placeholders
        );
        
        let mut attachment_stmt = self.conn.prepare(&attachment_query)?;
        let mut params = vec![account_email.to_string(), folder.to_string()];
        params.extend(uids);
        
        let attachment_rows = attachment_stmt.query_map(
            rusqlite::params_from_iter(params.iter()),
            |row| {
                let email_uid: u32 = row.get(0)?;
                let attachment = EmailAttachment {
                    filename: row.get(1)?,
                    content_type: row.get(2)?,
                    data: row.get(3)?,
                };
                Ok((email_uid, attachment))
            }
        )?;

        // Group attachments by email UID
        let mut attachments_by_uid: std::collections::HashMap<u32, Vec<EmailAttachment>> = 
            std::collections::HashMap::new();
        
        for attachment_result in attachment_rows {
            let (email_uid, attachment) = attachment_result?;
            attachments_by_uid.entry(email_uid).or_insert_with(Vec::new).push(attachment);
        }
        
        // Now build the final email objects
        let mut emails = Vec::new();
        
        for (uid, _message_id, subject, from_json, to_json, cc_json, bcc_json,
             date_timestamp, body_text, body_html, flags_str, headers_str, seen) in email_data {
            
            let from_addresses: Vec<EmailAddress> = 
                serde_json::from_str(&from_json).unwrap_or_default();
            let to_addresses: Vec<EmailAddress> = 
                serde_json::from_str(&to_json).unwrap_or_default();
            let cc_addresses: Vec<EmailAddress> = 
                serde_json::from_str(&cc_json).unwrap_or_default();
            let bcc_addresses: Vec<EmailAddress> = 
                serde_json::from_str(&bcc_json).unwrap_or_default();
            let flags: Vec<String> = 
                serde_json::from_str(&flags_str).unwrap_or_default();
            let headers: std::collections::HashMap<String, String> = 
                serde_json::from_str(&headers_str).unwrap_or_default();

            // Get attachments for this email (already loaded)
            let attachments = attachments_by_uid.remove(&uid).unwrap_or_default();

            let email = Email {
                id: uid.to_string(),
                subject,
                from: from_addresses,
                to: to_addresses,
                cc: cc_addresses,
                bcc: bcc_addresses,
                date: chrono::Local.timestamp_opt(date_timestamp, 0)
                    .single()
                    .unwrap_or_else(chrono::Local::now),
                body_text,
                body_html,
                attachments,
                flags,
                headers,
                seen,
                folder: folder.to_string(),
            };
            
            emails.push(email);
        }
        
        Ok(emails)
    }
}
