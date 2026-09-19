//! The benchmark corpus: where it lives, how it is divided into file classes,
//! and how a class is loaded into memory.
//!
//! The measuring itself is in `benches/corpus.rs`. Only the corpus lives here,
//! so a unit test can check that every class still resolves to the files
//! `corpus/README.md` says it has — a bench that silently reads nothing would
//! otherwise report a throughput number for an empty input.

use std::path::{Path, PathBuf};

/// `corpus/`, next to this crate's `Cargo.toml`.
pub const CORPUS_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/corpus");

/// A group of corpus files the benches report one throughput number for.
///
/// The classes are the ones in `corpus/README.md`. They overlap: `NoteHeavy`
/// is one file of `Plain`, and `WholeCorpus` is `Plain` plus the two synthetic
/// classes with nothing counted twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileClass {
    /// `web/` — 86 files, the whole World English Bible.
    Plain,
    /// `synthetic/attributes-heavy/` — 3 books, every word carries `\w`
    /// attributes.
    AttributesHeavy,
    /// `synthetic/alignment-heavy/` — Luke, every word inside
    /// `\zaln-s`/`\zaln-e`.
    AlignmentHeavy,
    /// `web/71-WIS.usfm` — the densest book in the corpus at 3.42 footnotes
    /// per KB. Already part of `Plain`.
    NoteHeavy,
    /// Everything: `Plain` + `AttributesHeavy` + `AlignmentHeavy`, 90 files.
    WholeCorpus,
}

impl FileClass {
    /// Every class, in the order the benches report them.
    pub const ALL: [FileClass; 5] = [
        FileClass::Plain,
        FileClass::AttributesHeavy,
        FileClass::AlignmentHeavy,
        FileClass::NoteHeavy,
        FileClass::WholeCorpus,
    ];

    /// The class's name, used as the criterion benchmark id.
    pub fn name(self) -> &'static str {
        match self {
            FileClass::Plain => "plain",
            FileClass::AttributesHeavy => "attributes-heavy",
            FileClass::AlignmentHeavy => "alignment-heavy",
            FileClass::NoteHeavy => "note-heavy",
            FileClass::WholeCorpus => "whole-corpus",
        }
    }

    /// The class's files, in a fixed order so two runs read the same bytes in
    /// the same sequence.
    ///
    /// # Panics
    ///
    /// If the corpus is missing or a class resolves to no files: a benchmark
    /// over nothing is worse than no benchmark.
    pub fn paths(self) -> Vec<PathBuf> {
        let root = Path::new(CORPUS_ROOT);
        let paths = match self {
            FileClass::Plain => usfm_files_in(&root.join("web")),
            FileClass::AttributesHeavy => {
                usfm_files_in(&root.join("synthetic").join("attributes-heavy"))
            }
            FileClass::AlignmentHeavy => {
                usfm_files_in(&root.join("synthetic").join("alignment-heavy"))
            }
            FileClass::NoteHeavy => {
                let path = root.join("web").join("71-WIS.usfm");
                assert!(path.is_file(), "{} is missing", path.display());
                vec![path]
            }
            FileClass::WholeCorpus => {
                let mut paths = FileClass::Plain.paths();
                paths.extend(FileClass::AttributesHeavy.paths());
                paths.extend(FileClass::AlignmentHeavy.paths());
                paths
            }
        };
        assert!(
            !paths.is_empty(),
            "corpus class {} resolved to no files under {CORPUS_ROOT}",
            self.name()
        );
        paths
    }

    /// The class's files, read into memory.
    pub fn load(self) -> Vec<CorpusFile> {
        self.paths()
            .into_iter()
            .map(|path| {
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|err| panic!("reading {}: {err}", path.display()));
                CorpusFile { path, text }
            })
            .collect()
    }
}

/// One corpus file held in memory. The benches time the parser, not the disk,
/// so every file is read before the timed loop starts.
#[derive(Debug, Clone)]
pub struct CorpusFile {
    pub path: PathBuf,
    pub text: String,
}

/// The total byte length of a loaded class. This is what criterion is told the
/// throughput is measured over, so the report reads as a rate (MiB/s) rather
/// than a time.
pub fn total_bytes(files: &[CorpusFile]) -> u64 {
    files.iter().map(|file| file.text.len() as u64).sum()
}

/// The `.usfm` files directly in `dir`, sorted by file name.
fn usfm_files_in(dir: &Path) -> Vec<PathBuf> {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|err| panic!("reading {}: {err}", dir.display()));
    let mut paths: Vec<PathBuf> = entries
        .map(|entry| entry.unwrap_or_else(|err| panic!("reading {}: {err}", dir.display())))
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "usfm"))
        .collect();
    paths.sort();
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The counts in `corpus/README.md`. A class that loses its files, or a
    /// regenerated corpus that gains some, fails here rather than quietly
    /// changing what a throughput number means.
    #[test]
    fn every_class_resolves_to_its_files() {
        for class in FileClass::ALL {
            let paths = class.paths();
            assert!(
                !paths.is_empty(),
                "class {} resolved to no files",
                class.name()
            );
        }
        assert_eq!(FileClass::Plain.paths().len(), 86);
        assert_eq!(FileClass::AttributesHeavy.paths().len(), 3);
        assert_eq!(FileClass::AlignmentHeavy.paths().len(), 1);
        assert_eq!(FileClass::NoteHeavy.paths().len(), 1);
        // 86 + 4, not double-counting note-heavy, which is one of the 86.
        assert_eq!(FileClass::WholeCorpus.paths().len(), 90);
    }

    #[test]
    fn the_whole_corpus_loads() {
        let files = FileClass::WholeCorpus.load();
        assert_eq!(files.len(), 90);
        // ~12.8 MB; a loose bound that only catches an empty or truncated read.
        assert!(total_bytes(&files) > 12_000_000);
    }
}
