# 53. Read a Paratext project: settings, book names, book files, stylesheet

Status: ready-for-agent
Milestone: after M7 (render gaps)

Found by mapping Shahkar-Urdu-Apps/render (gap 2). render reads a Paratext
project folder, not a file: `Settings.xml` for how books are named on disk
(`<Naming PrePart PostPart BookNameForm>`), which books are present
(`BooksPresent`), the stylesheet (`StyleSheet`, normally `usfm.sty`) and the
reference punctuation its `\xt` parser needs; `BookNames.xml` for each book's
abbreviation, short and long names. We read one file at a time.

- A new crate, `usfm_paratext`, on `usfm_style` and `roxmltree` (already the
  USX reader's XML parser), with no dependency on the parser: it finds and
  describes files, it does not parse books.
- `Project::open(dir)`: reads `Settings.xml` (required) and `BookNames.xml`
  (optional); `Settings` keeps every element as text and exposes the fields
  above typed; `book_names()`; `book_path(code)` from the naming rule
  (`41MAT` / `MAT` / `41` forms, Paratext's book numbers); `books()` from
  `BooksPresent`; `style_sheet()`: the default sheet with the project's
  `custom.sty` read over it (ticket 51's semantics). A `StyleSheet` setting
  other than `usfm.sty` is read in place of the default when the file is in
  the project, and falls back to the default otherwise.
- The facade re-exports it as `usfm::paratext` behind a `paratext` feature.
- Tests over a small fixture project written for the purpose (no real
  project data is committed).
