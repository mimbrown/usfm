# 63. Chapter selection and renumbering

Status: needs-triage
Milestone: after M7 (publishing foundation)

render's volumes pick chapters of a book in a given order and renumber
them (`SSV.INT 22, 13, 19, 14` becomes BAK, chapters 0–3). The usfm piece
is a function that takes chapters N, M, … of a `Document` and renumbers
their `ChapterStart`/`ChapterEnd`. Volumes themselves, as collections of
projects' books, stay in render. Triage with render's port.
