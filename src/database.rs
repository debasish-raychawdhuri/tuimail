use crate::email::{Email, EmailAttachment, EmailAddress, EmailSummary, debug_log};
use anyhow::{Result, Context};
use chrono::{DateTime, Local};
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

        // Add regular index on message_id for performance (not unique to allow duplicates)
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_message_id 
             ON emails(account_email, folder, message_id) 
             WHERE message_id IS NOT NULL",
            [],
        )?;

        // Migrate existing emails to extract Message-ID from headers
        self.migrate_message_ids()?;

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

        // Create email operations queue table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS email_operations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_email TEXT NOT NULL,
                operation_type TEXT NOT NULL, -- 'mark_read', 'mark_unread', 'delete', 'move'
                email_uid INTEGER NOT NULL,
                folder TEXT NOT NULL,
                target_folder TEXT, -- for move operations
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                processed BOOLEAN DEFAULT FALSE,
                error TEXT
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

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_email_operations_processed 
             ON email_operations(processed, created_at)",
            [],
        )?;

        // Drop the unique index if it exists (it causes constraint violations)
        self.conn.execute("DROP INDEX IF EXISTS idx_message_id", [])?;

        // Migrate existing emails to extract Message-ID from headers
        self.migrate_message_ids()?;

        Ok(())
    }

    /// Migrate existing emails to extract Message-ID from headers JSON
    fn migrate_message_ids(&self) -> Result<()> {
        // Check if migration is needed
        let null_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM emails WHERE message_id IS NULL",
            [],
            |row| row.get(0)
        )?;

        if null_count == 0 {
            return Ok(()); // No migration needed
        }

        debug_log(&format!("Migrating {} emails to extract Message-IDs from headers...", null_count));

        // Update emails with Message-ID extracted from headers JSON
        let updated = self.conn.execute(
            "UPDATE emails 
             SET message_id = json_extract(headers, '$.\"Message-ID\"')
             WHERE message_id IS NULL 
             AND json_extract(headers, '$.\"Message-ID\"') IS NOT NULL",
            []
        )?;

        debug_log(&format!("Successfully migrated {} emails with Message-IDs", updated));
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
                    Some(email.message_id()), // Extract Message-ID from headers
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
    pub fn get_email_count(&self, account_email: &str, folder: &str) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM emails WHERE account_email = ?1 AND folder = ?2",
            params![account_email, folder],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn get_latest_email_timestamp(&self, account_email: &str, folder: &str) -> Result<Option<i64>> {
        let result = self.conn.query_row(
            "SELECT MAX(date_received) FROM emails WHERE account_email = ?1 AND folder = ?2",
            params![account_email, folder],
            |row| row.get::<_, Option<i64>>(0)
        );
        
        match result {
            Ok(timestamp) => Ok(timestamp),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into())
        }
    }
    
    pub fn email_exists(&self, account_email: &str, folder: &str, uid: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM emails WHERE account_email = ?1 AND folder = ?2 AND uid = ?3",
            params![account_email, folder, uid],
            |row| row.get(0)
        )?;
        Ok(count > 0)
    }

    pub fn save_email(&self, email: &Email, account_email: &str) -> Result<()> {
        
        // Save the email
        self.conn.execute(
            "INSERT OR REPLACE INTO emails (
                uid, account_email, folder, message_id, subject,
                from_addresses, to_addresses, cc_addresses, bcc_addresses,
                date_received, body_text, body_html, flags, headers, seen
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                email.id.parse::<u32>().unwrap_or(0),
                account_email,
                email.folder,
                email.headers.get("Message-ID").unwrap_or(&email.id),
                email.subject,
                serde_json::to_string(&email.from)?,
                serde_json::to_string(&email.to)?,
                serde_json::to_string(&email.cc)?,
                serde_json::to_string(&email.bcc)?,
                email.date.timestamp(),
                email.body_text,
                email.body_html,
                serde_json::to_string(&email.flags)?,
                serde_json::to_string(&email.headers)?,
                email.seen,
            ],
        )?;

        // Save attachments
        for attachment in &email.attachments {
            self.conn.execute(
                "INSERT OR REPLACE INTO attachments (account_email, folder, email_uid, filename, content_type, data, size)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    account_email,
                    email.folder,
                    email.id.parse::<u32>().unwrap_or(0),
                    attachment.filename,
                    attachment.content_type,
                    attachment.data,
                    attachment.data.len() as i64,
                ],
            )?;
        }

        Ok(())
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


    /// Load lightweight email summaries for list display (no body, no attachment data)
    pub fn get_recent_email_summaries(&self, account_email: &str, folder: &str, limit: usize) -> Result<Vec<EmailSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.uid, e.subject, e.from_addresses, e.date_received, e.seen,
                    EXISTS(SELECT 1 FROM attachments a WHERE a.account_email = e.account_email AND a.folder = e.folder AND a.email_uid = e.uid) as has_attachments
             FROM emails e
             WHERE e.account_email = ?1 AND e.folder = ?2
             ORDER BY e.date_received DESC
             LIMIT ?3",
        )?;

        let rows = stmt.query_map(params![account_email, folder, limit], |row| {
            Ok((
                row.get::<_, u32>(0)?,       // uid
                row.get::<_, String>(1)?,    // subject
                row.get::<_, String>(2)?,    // from_addresses
                row.get::<_, i64>(3)?,       // date_received
                row.get::<_, bool>(4)?,      // seen
                row.get::<_, bool>(5)?,      // has_attachments
            ))
        })?;

        let mut summaries = Vec::new();
        for row_result in rows {
            let (uid, subject, from_str, date_received, seen, has_attachments) = row_result?;
            let from: Vec<EmailAddress> = serde_json::from_str(&from_str).unwrap_or_default();

            summaries.push(EmailSummary {
                id: uid.to_string(),
                subject,
                from,
                date: chrono::DateTime::from_timestamp(date_received, 0)
                    .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap())
                    .with_timezone(&Local),
                seen,
                folder: folder.to_string(),
                has_attachments,
            });
        }

        Ok(summaries)
    }

    /// Load a full email by UID (body + attachments with data) for viewing
    pub fn get_email_full(&self, account_email: &str, folder: &str, uid: u32) -> Result<Email> {
        let row = self.conn.query_row(
            "SELECT uid, subject, from_addresses, to_addresses,
                    cc_addresses, bcc_addresses, date_received, body_text, body_html,
                    flags, headers, seen
             FROM emails
             WHERE account_email = ?1 AND folder = ?2 AND uid = ?3",
            params![account_email, folder, uid],
            |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, bool>(11)?,
                ))
            },
        )?;

        let (uid, subject, from_str, to_str, cc_str, bcc_str, date_received,
             body_text, body_html, flags_str, headers_str, seen) = row;

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
        for a in attachment_rows {
            attachments.push(a?);
        }

        Ok(Email {
            id: uid.to_string(),
            subject,
            from: serde_json::from_str(&from_str).unwrap_or_default(),
            to: serde_json::from_str(&to_str).unwrap_or_default(),
            cc: serde_json::from_str(&cc_str).unwrap_or_default(),
            bcc: serde_json::from_str(&bcc_str).unwrap_or_default(),
            date: chrono::DateTime::from_timestamp(date_received, 0)
                .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap())
                .with_timezone(&Local),
            body_text,
            body_html,
            attachments,
            flags: serde_json::from_str(&flags_str).unwrap_or_default(),
            headers: serde_json::from_str(&headers_str).unwrap_or_default(),
            seen,
            folder: folder.to_string(),
        })
    }

    pub fn update_email_seen_status(&self, account_email: &str, folder: &str, uid: u32, seen: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE emails SET seen = ?1, updated_at = strftime('%s', 'now') 
             WHERE account_email = ?2 AND folder = ?3 AND uid = ?4",
            params![seen, account_email, folder, uid],
        )?;
        Ok(())
    }

    pub fn delete_email(&self, account_email: &str, folder: &str, uid: u32) -> Result<()> {
        self.conn.execute(
            "DELETE FROM emails WHERE account_email = ?1 AND folder = ?2 AND uid = ?3",
            params![account_email, folder, uid],
        )?;
        Ok(())
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
    
    /// Search emails by subject, from, or body text
    pub fn search_emails(&self, account_email: &str, query: &str, limit: usize) -> Result<Vec<Email>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT uid, message_id, subject, from_addresses, to_addresses,
                    cc_addresses, bcc_addresses, date_received, body_text, body_html,
                    flags, headers, seen, folder
             FROM emails
             WHERE account_email = ?1
               AND (subject LIKE ?2 OR from_addresses LIKE ?2 OR body_text LIKE ?2 OR to_addresses LIKE ?2)
             ORDER BY date_received DESC
             LIMIT ?3",
        )?;

        let email_rows = stmt.query_map(params![account_email, pattern, limit], |row| {
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, bool>(12)?,
                row.get::<_, String>(13)?,
            ))
        })?;

        let mut emails = Vec::new();
        for row_result in email_rows {
            let (uid, _message_id, subject, from_json, to_json, cc_json, bcc_json,
                 date_timestamp, body_text, body_html, flags_json, headers_json, seen, folder) = row_result?;

            let email = Email {
                id: uid.to_string(),
                subject,
                from: serde_json::from_str(&from_json).unwrap_or_default(),
                to: serde_json::from_str(&to_json).unwrap_or_default(),
                cc: serde_json::from_str(&cc_json).unwrap_or_default(),
                bcc: serde_json::from_str(&bcc_json).unwrap_or_default(),
                date: chrono::DateTime::from_timestamp(date_timestamp, 0)
                    .unwrap_or_else(|| chrono::Local::now().into())
                    .with_timezone(&chrono::Local),
                body_text,
                body_html,
                attachments: Vec::new(),
                flags: serde_json::from_str(&flags_json).unwrap_or_default(),
                headers: serde_json::from_str(&headers_json).unwrap_or_default(),
                seen,
                folder,
            };
            emails.push(email);
        }
        Ok(emails)
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_db() -> EmailDatabase {
        let db = EmailDatabase::new(std::path::Path::new(":memory:")).unwrap();
        // Create sync_state table (normally created by main.rs or sync daemon)
        db.conn.execute(
            "CREATE TABLE IF NOT EXISTS sync_state (
                account_email TEXT NOT NULL,
                folder TEXT NOT NULL,
                last_uid_seen INTEGER NOT NULL DEFAULT 0,
                last_sync_timestamp INTEGER NOT NULL DEFAULT 0,
                sync_in_progress BOOLEAN NOT NULL DEFAULT FALSE,
                PRIMARY KEY(account_email, folder)
            )",
            [],
        ).unwrap();
        db
    }

    fn make_email(id: &str, subject: &str, folder: &str) -> Email {
        Email {
            id: id.to_string(),
            subject: subject.to_string(),
            from: vec![EmailAddress { name: Some("Sender".to_string()), address: "sender@test.com".to_string() }],
            to: vec![EmailAddress { name: None, address: "recipient@test.com".to_string() }],
            cc: vec![],
            bcc: vec![],
            date: chrono::Local::now(),
            body_text: Some("Test body".to_string()),
            body_html: None,
            attachments: vec![],
            flags: vec!["\\Seen".to_string()],
            headers: HashMap::new(),
            seen: true,
            folder: folder.to_string(),
        }
    }

    #[test]
    fn test_database_creation() {
        let db = test_db();
        assert!(db.get_database_path().contains("memory"));
    }

    #[test]
    fn test_save_and_load_email() {
        let db = test_db();
        let email = make_email("1", "Test Subject", "INBOX");
        db.save_email(&email, "user@test.com").unwrap();

        let loaded = db.load_emails("user@test.com", "INBOX").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].subject, "Test Subject");
        assert_eq!(loaded[0].id, "1");
    }

    #[test]
    fn test_save_emails_batch() {
        let db = test_db();
        let emails = vec![
            make_email("1", "First", "INBOX"),
            make_email("2", "Second", "INBOX"),
            make_email("3", "Third", "INBOX"),
        ];
        db.save_emails("user@test.com", "INBOX", &emails).unwrap();
        let count = db.get_email_count("user@test.com", "INBOX").unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_email_exists() {
        let db = test_db();
        assert!(!db.email_exists("user@test.com", "INBOX", "1").unwrap());
        let email = make_email("1", "Test", "INBOX");
        db.save_email(&email, "user@test.com").unwrap();
        assert!(db.email_exists("user@test.com", "INBOX", "1").unwrap());
    }

    #[test]
    fn test_email_count_empty() {
        let db = test_db();
        assert_eq!(db.get_email_count("user@test.com", "INBOX").unwrap(), 0);
    }

    #[test]
    fn test_email_count_multiple_folders() {
        let db = test_db();
        db.save_email(&make_email("1", "A", "INBOX"), "u@t.com").unwrap();
        db.save_email(&make_email("2", "B", "INBOX"), "u@t.com").unwrap();
        db.save_email(&make_email("3", "C", "Sent"), "u@t.com").unwrap();
        assert_eq!(db.get_email_count("u@t.com", "INBOX").unwrap(), 2);
        assert_eq!(db.get_email_count("u@t.com", "Sent").unwrap(), 1);
    }

    #[test]
    fn test_save_email_with_attachment() {
        let db = test_db();
        let mut email = make_email("1", "With Attachment", "INBOX");
        email.attachments.push(EmailAttachment {
            filename: "test.txt".to_string(),
            content_type: "text/plain".to_string(),
            data: b"hello world".to_vec(),
        });
        db.save_email(&email, "user@test.com").unwrap();

        let loaded = db.load_emails("user@test.com", "INBOX").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].attachments.len(), 1);
        assert_eq!(loaded[0].attachments[0].filename, "test.txt");
        assert_eq!(loaded[0].attachments[0].data, b"hello world");
    }

    #[test]
    fn test_save_email_replaces_existing() {
        let db = test_db();
        db.save_email(&make_email("1", "Original", "INBOX"), "u@t.com").unwrap();
        db.save_email(&make_email("1", "Updated", "INBOX"), "u@t.com").unwrap();
        let loaded = db.load_emails("u@t.com", "INBOX").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].subject, "Updated");
    }

    #[test]
    fn test_delete_emails_by_folder() {
        let db = test_db();
        db.save_email(&make_email("1", "A", "INBOX"), "u@t.com").unwrap();
        db.save_email(&make_email("2", "B", "Sent"), "u@t.com").unwrap();
        db.delete_emails_by_folder("u@t.com", "INBOX").unwrap();
        assert_eq!(db.get_email_count("u@t.com", "INBOX").unwrap(), 0);
        assert_eq!(db.get_email_count("u@t.com", "Sent").unwrap(), 1);
    }

    #[test]
    fn test_folder_metadata() {
        let db = test_db();
        let (uid, total, sync) = db.load_folder_metadata("u@t.com", "INBOX").unwrap();
        assert_eq!(uid, 0);
        assert_eq!(total, 0);
        assert_eq!(sync, 0);

        db.save_folder_metadata("u@t.com", "INBOX", 100, 50).unwrap();
        let (uid, total, _sync) = db.load_folder_metadata("u@t.com", "INBOX").unwrap();
        assert_eq!(uid, 100);
        assert_eq!(total, 50);
    }

    #[test]
    fn test_queue_and_get_operations() {
        let db = test_db();
        db.queue_email_operation("u@t.com", "mark_read", 1, "INBOX", None).unwrap();
        db.queue_email_operation("u@t.com", "move", 2, "INBOX", Some("Archive")).unwrap();

        let ops = db.get_pending_operations().unwrap();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].2, "mark_read");
        assert_eq!(ops[1].2, "move");
        assert_eq!(ops[1].5.as_deref(), Some("Archive"));
    }

    #[test]
    fn test_mark_operation_processed() {
        let db = test_db();
        db.queue_email_operation("u@t.com", "mark_read", 1, "INBOX", None).unwrap();
        let ops = db.get_pending_operations().unwrap();
        assert_eq!(ops.len(), 1);
        db.mark_operation_processed(ops[0].0).unwrap();
        let ops = db.get_pending_operations().unwrap();
        assert_eq!(ops.len(), 0);
    }

    #[test]
    fn test_vacuum() {
        let db = test_db();
        db.vacuum().unwrap();
    }

    #[test]
    fn test_get_database_size() {
        let db = test_db();
        let size = db.get_database_size().unwrap();
        assert!(size > 0);
    }

    #[test]
    fn test_get_latest_email_timestamp_empty() {
        let db = test_db();
        assert_eq!(db.get_latest_email_timestamp("u@t.com", "INBOX").unwrap(), None);
    }

    #[test]
    fn test_seen_status_persists_after_reload() {
        // This tests the exact bug: mark email as read, reload from DB,
        // verify it's still marked as read (not reset to unread).
        let db = test_db();
        let email = make_email("1", "Test", "INBOX");
        assert!(email.seen); // make_email creates seen=true by default

        // Save as unseen
        let mut unseen_email = email.clone();
        unseen_email.seen = false;
        unseen_email.flags = vec![];
        db.save_email(&unseen_email, "u@t.com").unwrap();

        // Verify it loads as unseen
        let loaded = db.load_emails("u@t.com", "INBOX").unwrap();
        assert!(!loaded[0].seen, "Email should start as unseen");

        // Mark as seen via update_email_seen_status (this is what mark_current_email_as_read does)
        db.update_email_seen_status("u@t.com", "INBOX", 1, true).unwrap();

        // Reload from database (simulates switching away and back to folder)
        let reloaded = db.load_emails("u@t.com", "INBOX").unwrap();
        assert!(reloaded[0].seen, "Email should still be seen after reload from DB");
    }

    #[test]
    fn test_seen_status_must_update_same_db_it_reads_from() {
        // This reproduces the root cause: if seen status is updated in one DB
        // but emails are loaded from a different DB, the status is lost.
        let db1 = test_db(); // simulates the main database
        let db2 = test_db(); // simulates the per-account database

        let mut email = make_email("1", "Test", "INBOX");
        email.seen = false;
        email.flags = vec![];

        // Both databases start with the same unseen email
        db1.save_email(&email, "u@t.com").unwrap();
        db2.save_email(&email, "u@t.com").unwrap();

        // BUG scenario: only update db1 (main), but load from db2 (per-account)
        db1.update_email_seen_status("u@t.com", "INBOX", 1, true).unwrap();
        let from_db2 = db2.load_emails("u@t.com", "INBOX").unwrap();
        assert!(!from_db2[0].seen, "Without fix: db2 still shows unseen because only db1 was updated");

        // FIX scenario: update BOTH databases
        db2.update_email_seen_status("u@t.com", "INBOX", 1, true).unwrap();
        let from_db2_fixed = db2.load_emails("u@t.com", "INBOX").unwrap();
        assert!(from_db2_fixed[0].seen, "With fix: db2 now shows seen because both DBs were updated");
    }

    #[test]
    fn test_search_by_subject() {
        let db = test_db();
        db.save_email(&make_email("1", "Meeting tomorrow", "INBOX"), "u@t.com").unwrap();
        db.save_email(&make_email("2", "Lunch plans", "INBOX"), "u@t.com").unwrap();
        db.save_email(&make_email("3", "Meeting notes", "Sent"), "u@t.com").unwrap();

        let results = db.search_emails("u@t.com", "meeting", 100).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|e| e.subject.to_lowercase().contains("meeting")));
    }

    #[test]
    fn test_search_by_body() {
        let db = test_db();
        let mut email = make_email("1", "Hello", "INBOX");
        email.body_text = Some("Let's discuss the budget report".to_string());
        db.save_email(&email, "u@t.com").unwrap();
        db.save_email(&make_email("2", "Other", "INBOX"), "u@t.com").unwrap();

        let results = db.search_emails("u@t.com", "budget", 100).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].subject, "Hello");
    }

    #[test]
    fn test_search_no_results() {
        let db = test_db();
        db.save_email(&make_email("1", "Hello", "INBOX"), "u@t.com").unwrap();
        let results = db.search_emails("u@t.com", "zzzznonexistent", 100).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_respects_limit() {
        let db = test_db();
        for i in 1..=10 {
            db.save_email(&make_email(&i.to_string(), &format!("Test email {}", i), "INBOX"), "u@t.com").unwrap();
        }
        let results = db.search_emails("u@t.com", "Test", 3).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_search_across_folders() {
        let db = test_db();
        db.save_email(&make_email("1", "Project update", "INBOX"), "u@t.com").unwrap();
        db.save_email(&make_email("2", "Project review", "Sent"), "u@t.com").unwrap();
        db.save_email(&make_email("3", "Unrelated", "INBOX"), "u@t.com").unwrap();

        let results = db.search_emails("u@t.com", "project", 100).unwrap();
        assert_eq!(results.len(), 2);
        // Results should include folder info
        let folders: Vec<&str> = results.iter().map(|e| e.folder.as_str()).collect();
        assert!(folders.contains(&"INBOX"));
        assert!(folders.contains(&"Sent"));
    }

    #[test]
    fn test_search_case_insensitive() {
        let db = test_db();
        db.save_email(&make_email("1", "URGENT Meeting", "INBOX"), "u@t.com").unwrap();
        let results = db.search_emails("u@t.com", "urgent", 100).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_mark_unread_persists_after_reload() {
        let db = test_db();
        let email = make_email("1", "Test", "INBOX");
        db.save_email(&email, "u@t.com").unwrap();

        // Start as seen (make_email default)
        let loaded = db.load_emails("u@t.com", "INBOX").unwrap();
        assert!(loaded[0].seen);

        // Mark as unread
        db.update_email_seen_status("u@t.com", "INBOX", 1, false).unwrap();

        // Reload and verify
        let reloaded = db.load_emails("u@t.com", "INBOX").unwrap();
        assert!(!reloaded[0].seen, "Email should be unseen after marking unread and reloading");
    }
}
