use tokio::{fs, sync::Mutex};
use tower_lsp_server::{
    UriExt,
    lsp_types::{FileSystemWatcher, GlobPattern, OneOf, RelativePattern, Uri, WatchKind},
};

use crate::Options;

pub struct WorkspaceWorker {
    root_uri: Uri,
    options: Mutex<Options>,
}

impl WorkspaceWorker {
    pub fn new(root_uri: Uri) -> Self {
        Self {
            root_uri,
            options: Mutex::new(Options::default()),
        }
    }

    async fn init_config(&self) {
        let Some(root_path) = self.root_uri.to_file_path() else {
            return;
        };
        // let mut config_path = None;
        let config_path = root_path.join(&self.options.lock().await.config_path);
        let Some(contents) = fs::read_to_string(config_path).await.ok() else {
            return;
        };
        let Some(config) = serde_json::from_str::<UsfmConfig>(&contents).ok() else {
            return;
        };
        if let Some(lexicon_options) = config.lexicon {
            let db_path = root_path.join(&lexicon_options.path);
            rusqlite_regex::enable_auto_extension().ok();
            if let Ok(connection) = rusqlite::Connection::open_with_flags(
                db_path,
                OpenFlags::SQLITE_OPEN_READ_WRITE
                // | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_URI,
            ) {
                if let Ok(db) = DataLayer::init(connection) {
                    *self.db.lock().await = Some(db);
                }
            }
        }
        // let config = serde_json::from_value::<UsfmConfig>(contents).ok();
        // if config.exists() {
        //     config_path = Some(config);
        // }
        // if let Some(config_path) = config_path {
        //     let mut linter = self.server_linter.write().await;
        //     *linter = ServerLinter::new_with_linter(
        //         LinterBuilder::from_oxlintrc(
        //             true,
        //             Oxlintrc::from_file(&config_path)
        //                 .expect("should have initialized linter with new options"),
        //         )
        //         // FIXME: Handle this error more gracefully and report it properly
        //         .expect("failed to build linter from oxlint config")
        //         .with_fix(FixKind::SafeFix)
        //         .build(),
        //     );
        // }
    }

    pub fn get_root_uri(&self) -> &Uri {
        &self.root_uri
    }

    pub fn is_responsible_for_uri(&self, uri: &Uri) -> bool {
        if let Some(path) = uri.to_file_path() {
            return path.starts_with(self.root_uri.to_file_path().unwrap());
        }
        false
    }

    // WARNING: start all programs (linter, formatter) before calling this function
    // each program can tell us customized file watcher patterns
    pub async fn init_watchers(&self) -> Vec<FileSystemWatcher> {
        let mut watchers = Vec::new();

        // clone the options to avoid locking the mutex
        let options = self.options.lock().await;

        // append the base watcher
        watchers.push(FileSystemWatcher {
            glob_pattern: GlobPattern::Relative(RelativePattern {
                base_uri: OneOf::Right(self.root_uri.clone()),
                pattern: options
                    .config_path
                    .as_ref()
                    .unwrap_or(&"**/usfm.config.json".to_owned())
                    .to_owned(),
            }),
            kind: Some(WatchKind::all()), // created, deleted, changed
        });

        watchers
    }
}

// fn range_overlaps(a: Range, b: Range) -> bool {
//     a.start <= b.end && a.end >= b.start
// }

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_get_root_uri() {
        let worker = WorkspaceWorker::new(Uri::from_str("file:///root/").unwrap());

        assert_eq!(
            worker.get_root_uri(),
            &Uri::from_str("file:///root/").unwrap()
        );
    }

    #[test]
    fn test_is_responsible() {
        let worker = WorkspaceWorker::new(Uri::from_str("file:///path/to/root").unwrap());

        assert!(
            worker.is_responsible_for_uri(&Uri::from_str("file:///path/to/root/file.js").unwrap())
        );
        assert!(worker.is_responsible_for_uri(
            &Uri::from_str("file:///path/to/root/folder/file.js").unwrap()
        ));
        assert!(
            !worker
                .is_responsible_for_uri(&Uri::from_str("file:///path/to/other/file.js").unwrap())
        );
    }
}
