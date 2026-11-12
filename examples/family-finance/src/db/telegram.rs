//! The Telegram chats allowed to talk to the app, and the QR codes that pair them.

use rusqlite::{OptionalExtension, params};

use super::{Database, DbError, DbResult};
use crate::model::Member;

impl Database {
    /// Replaces every unclaimed QR code with one short-lived pairing request, for the chat
    /// of `member` (or of no one in particular). Chats already paired stay.
    pub fn begin_telegram_pairing(&self, token_hash: &str, member: Option<i64>, now: i64) -> DbResult<()> {
        if token_hash.len() != 64 || !token_hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(DbError::Rule("O código de pareamento é inválido.".to_owned()));
        }
        self.atomically(|db| {
            db.conn
                .execute("UPDATE telegram_pairings SET status = 'expired' WHERE status = 'pending'", [])?;
            db.conn.execute(
                "INSERT INTO telegram_pairings (token_hash, member_id, status, created_at, expires_at)
                 VALUES (?1, ?3, 'pending', ?2, ?2 + 600)",
                params![token_hash, now, member],
            )?;
            Ok(())
        })
    }

    /// Consumes a valid QR code and links this private Telegram chat, beside the chats
    /// already linked. A chat that pairs again keeps only its new link.
    pub fn claim_telegram_pairing(
        &self,
        token_hash: &str,
        chat_id: i64,
        user_id: i64,
        now: i64,
    ) -> DbResult<bool> {
        if chat_id == 0
            || user_id == 0
            || token_hash.len() != 64
            || !token_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Ok(false);
        }
        self.atomically(|db| {
            db.conn.execute(
                "UPDATE telegram_pairings SET status = 'expired' WHERE status = 'pending' AND expires_at <= ?1",
                [now],
            )?;
            let valid: Option<i64> = db.conn.query_row(
                "SELECT id FROM telegram_pairings WHERE token_hash = ?1 AND status = 'pending' AND expires_at > ?2",
                params![token_hash, now],
                |row| row.get(0),
            )
            .optional()?;
            if valid.is_none() {
                return Ok(false);
            }
            db.conn.execute(
                "UPDATE telegram_pairings
                 SET status = 'revoked', revoked_at = ?1, chat_id = NULL, user_id = NULL
                 WHERE status = 'active' AND chat_id = ?2",
                params![now, chat_id],
            )?;
            db.conn.execute(
                "UPDATE telegram_pairings SET chat_id = ?2, user_id = ?3, status = 'active', paired_at = ?4
                 WHERE token_hash = ?1",
                params![token_hash, chat_id, user_id, now],
            )?;
            Ok(true)
        })
    }

    /// Whether this private chat has an active Telegram link to this database.
    pub fn telegram_chat_is_linked(&self, chat_id: i64, user_id: i64) -> DbResult<bool> {
        Ok(self
            .conn
            .query_row(
                "SELECT 1 FROM telegram_pairings WHERE chat_id = ?1 AND user_id = ?2 AND status = 'active'",
                params![chat_id, user_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    /// Revokes one chat's link and every still-unclaimed QR code. The other chats stay.
    pub fn revoke_telegram_chat(&self, chat_id: i64, now: i64) -> DbResult<()> {
        self.atomically(|db| {
            db.conn.execute(
                "UPDATE telegram_pairings
                 SET status = 'revoked', revoked_at = ?1, chat_id = NULL, user_id = NULL
                 WHERE status = 'active' AND chat_id = ?2",
                params![now, chat_id],
            )?;
            db.conn
                .execute("UPDATE telegram_pairings SET status = 'expired' WHERE status = 'pending'", [])?;
            Ok(())
        })
    }

    /// The chats linked now, with the member each was made for, oldest first.
    pub fn telegram_links(&self) -> DbResult<Vec<TelegramLink>> {
        let mut query = self.conn.prepare_cached(
            "SELECT p.chat_id, p.member_id, m.name, p.paired_at FROM telegram_pairings p
             LEFT JOIN members m ON m.id = p.member_id
             WHERE p.status = 'active' AND p.chat_id IS NOT NULL ORDER BY p.paired_at, p.id",
        )?;
        let rows = query.query_map([], |r| {
            Ok(TelegramLink {
                chat_id: r.get(0)?,
                member: match (r.get::<_, Option<i64>>(1)?, r.get::<_, Option<String>>(2)?) {
                    (Some(id), Some(name)) => Some(Member { id, name }),
                    _ => None,
                },
                paired_at: r.get::<_, Option<i64>>(3)?.unwrap_or_default(),
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Whether a QR made here still waits to be scanned.
    pub fn telegram_qr_waiting(&self, now: i64) -> DbResult<bool> {
        Ok(self
            .conn
            .query_row(
                "SELECT 1 FROM telegram_pairings WHERE status = 'pending' AND expires_at > ?1",
                [now],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    /// The member this linked chat speaks for, if its QR was made for one.
    pub fn telegram_chat_member(&self, chat_id: i64) -> DbResult<Option<Member>> {
        Ok(self
            .telegram_links()?
            .into_iter()
            .find(|link| link.chat_id == chat_id)
            .and_then(|link| link.member))
    }

    /// The private Telegram chats paired now, which the bot's notices go to.
    pub fn telegram_chats(&self) -> DbResult<Vec<i64>> {
        let mut query = self.conn.prepare_cached(
            "SELECT chat_id FROM telegram_pairings WHERE status = 'active' AND chat_id IS NOT NULL",
        )?;
        let rows = query.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Whether one private Telegram chat is paired now.
    pub fn has_telegram_pairing(&self) -> DbResult<bool> {
        Ok(self
            .conn
            .query_row("SELECT 1 FROM telegram_pairings WHERE status = 'active'", [], |_| Ok(()))
            .optional()?
            .is_some())
    }
}

/// A private Telegram chat linked to this database.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TelegramLink {
    /// Telegram's chat id.
    pub chat_id: i64,
    /// The member the chat speaks for; `None` when its QR was made for no one in particular.
    pub member: Option<Member>,
    /// When it paired, in Unix seconds.
    pub paired_at: i64,
}
