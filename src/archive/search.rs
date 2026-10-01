//! The full-text index behind global message search.
//!
//! Searching every body with `LIKE '%..%'` reads and parses each message's
//! JSON on every keystroke, so its cost grew with the whole history and a
//! search with no hits was the slowest of all. A trigram index answers the
//! same substring queries ("day" still finds "birthday") from the index.
//! It is contentless: it keeps no second copy of the text, only the trigrams,
//! keyed by the message's rowid. Triggers keep it in step with every insert,
//! edit, and deletion.

use rusqlite::Connection;

use super::Result;

/// The searchable text of a message's JSON `content` column: the text the
/// earlier scan matched, [`super::SEARCHED_TEXT`], read from `content`.
fn body(content: &str) -> String {
    let fields = super::SEARCHED_TEXT.replace("content", content);
    format!("CASE WHEN json_valid({content}) THEN {fields} END")
}

pub(super) fn schema() -> String {
    let body = body("new.content");
    format!(
        "CREATE VIRTUAL TABLE IF NOT EXISTS message_text USING fts5(
            body, content = '', contentless_delete = 1,
            tokenize = 'trigram case_sensitive 0 remove_diacritics 1'
        );
        CREATE TRIGGER IF NOT EXISTS message_text_insert AFTER INSERT ON messages BEGIN
            INSERT INTO message_text (rowid, body) VALUES (new.rowid, {body});
        END;
        CREATE TRIGGER IF NOT EXISTS message_text_update AFTER UPDATE OF content ON messages
        WHEN old.content IS NOT new.content BEGIN
            DELETE FROM message_text WHERE rowid = old.rowid;
            INSERT INTO message_text (rowid, body) VALUES (new.rowid, {body});
        END;
        CREATE TRIGGER IF NOT EXISTS message_text_delete AFTER DELETE ON messages BEGIN
            DELETE FROM message_text WHERE rowid = old.rowid;
        END;"
    )
}

/// Bumped when the indexed text changes, to index the archive again.
const VERSION_KEY: &str = "message_text_v1";

/// Indexes the messages filed before the index existed, once.
pub(super) fn backfill(connection: &Connection) -> Result<()> {
    let done: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM meta WHERE key = ?1)",
        [VERSION_KEY],
        |row| row.get(0),
    )?;
    if done {
        return Ok(());
    }
    let transaction = connection.unchecked_transaction()?;
    transaction.execute_batch(&format!(
        "INSERT INTO message_text (message_text) VALUES ('delete-all');
         INSERT INTO message_text (rowid, body) SELECT rowid, {} FROM messages;",
        body("content")
    ))?;
    transaction.execute(
        "INSERT INTO meta (key, value) VALUES (?1, 'indexed')",
        [VERSION_KEY],
    )?;
    transaction.commit()
}

/// The index query for `needle`, or `None` when it is too short for the
/// index: a trigram index cannot answer fewer than three characters.
pub(super) fn query(needle: &str) -> Option<String> {
    let needle = needle.trim();
    (needle.chars().count() >= 3).then(|| format!("\"{}\"", needle.replace('"', "\"\"")))
}
