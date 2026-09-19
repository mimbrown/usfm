//! `--watch`: rebuild whenever one of the run's files changes.
//!
//! Moved here from the old `usfm_parser/src/main.rs` with the `notify` dependency
//! (ticket 15); the parser is a library again and watches nothing.

use std::path::Path;
use std::sync::mpsc;

use notify::event::ModifyKind;
use notify::{Event, EventKind, RecursiveMode, Watcher};

use crate::driver::Driver;
use crate::error::Error;

/// Write the output once, then again after every change to a watched file.
/// Returns only if the watch channel closes; Ctrl+C is the usual way out.
///
/// A failed run is reported and the loop goes on: the next save is very
/// likely the fix.
pub fn run(driver: &mut Driver) -> Result<(), Error> {
    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = notify::recommended_watcher(tx).map_err(std::io::Error::other)?;

    for path in driver.watched_paths() {
        if let Ok(path) = Path::new(&path).canonicalize() {
            watcher
                .watch(&path, RecursiveMode::NonRecursive)
                .map_err(std::io::Error::other)?;
        }
    }

    if let Err(e) = driver.run() {
        e.print();
    }

    // Only when the output is a file: on stdout this line would land in the
    // middle of the document.
    if driver.writes_to_file() {
        println!("Watching for changes... Press Ctrl+C to exit");
    }

    for res in rx {
        match res {
            Ok(event) => {
                if is_content_change(&event) {
                    if let Err(errors) = driver.paths_updated(&event.paths) {
                        for error in errors {
                            error.print();
                        }
                        continue;
                    }
                    if let Err(e) = driver.run() {
                        e.print();
                    }
                }
            }
            Err(e) => eprintln!("Watch error: {:?}", e),
        }
    }

    Ok(())
}

/// A change to what a file holds, rather than to when it was last read.
fn is_content_change(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
    ) && !matches!(event.kind, EventKind::Modify(ModifyKind::Metadata(_)))
}
