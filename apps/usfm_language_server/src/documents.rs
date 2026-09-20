//! The text of every document the client has open.
//!
//! Text synchronisation is `FULL` (see [`crate::Backend::initialize`]), so an
//! entry is replaced whole on every change and there is no incremental patch
//! to apply. Diagnostics do not even need the store — `did_open` and
//! `did_change` are handed the new text — but formatting, hover and the rest
//! of M6 (tickets 31 and 32) are requests that arrive with a position and no
//! text, so the store is what answers them.
//!
//! It is a `HashMap` behind a `tokio::sync::RwLock` rather than a `DashMap`:
//! the map is touched once per notification and the critical section is an
//! insert or a clone, so there is nothing for a sharded map to relieve — and
//! it is one dependency fewer in a workspace that had no async dependency at
//! all before this crate.

use std::collections::HashMap;

use tokio::sync::RwLock;
use tower_lsp_server::ls_types::Uri;

/// The open documents, by URI.
#[derive(Debug, Default)]
pub struct Documents {
    texts: RwLock<HashMap<Uri, String>>,
}

impl Documents {
    /// Record `text` as the whole content of `uri`, replacing what was there.
    pub async fn set(&self, uri: &Uri, text: String) {
        self.texts.write().await.insert(uri.clone(), text);
    }

    /// Forget `uri`, which the client has closed.
    pub async fn remove(&self, uri: &Uri) {
        self.texts.write().await.remove(uri);
    }

    /// The text of `uri`, if it is open.
    ///
    /// A clone, because the caller parses it and a `ParseResult` borrows the
    /// source for as long as the tree lives: holding the lock that long would
    /// block every other notification.
    pub async fn text(&self, uri: &Uri) -> Option<String> {
        self.texts.read().await.get(uri).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::str::FromStr;

    fn uri(path: &str) -> Uri {
        Uri::from_str(path).expect("a valid URI")
    }

    #[tokio::test]
    async fn a_document_is_kept_until_it_is_closed() {
        let documents = Documents::default();
        let genesis = uri("file:///books/01GEN.SFM");
        let exodus = uri("file:///books/02EXO.SFM");

        assert_eq!(documents.text(&genesis).await, None);

        documents.set(&genesis, "\\id GEN\n".to_owned()).await;
        documents.set(&exodus, "\\id EXO\n".to_owned()).await;
        assert_eq!(
            documents.text(&genesis).await.as_deref(),
            Some("\\id GEN\n")
        );

        // A change replaces the whole text.
        documents
            .set(&genesis, "\\id GEN\n\\c 1\n".to_owned())
            .await;
        assert_eq!(
            documents.text(&genesis).await.as_deref(),
            Some("\\id GEN\n\\c 1\n")
        );

        // Closing one leaves the other.
        documents.remove(&genesis).await;
        assert_eq!(documents.text(&genesis).await, None);
        assert_eq!(documents.text(&exodus).await.as_deref(), Some("\\id EXO\n"));
    }
}
