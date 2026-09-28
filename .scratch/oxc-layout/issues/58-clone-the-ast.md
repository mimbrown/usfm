# 58. `Clone` for `Document` and every AST node

Status: resolved (2026-09-28)
Milestone: after M7 (publishing foundation)

No node in `usfm_ast` is `Clone`. The first draft of the publishing
foundation split one edited `Document` into a print copy and a digital
copy. The spec has since settled on borrowing instead (see the Answer),
so this is a convenience, not a requirement.

Derive `Clone` on `Document` and on every node type. The derive is
additive: nothing about the tree's shape changes. A `Document` owns its
sheet through an `Arc`, so a clone shares the sheet, and a clone of a
borrowed parse borrows the same source.

Done when a clone of every conformance case's parse is equal to the
original (`eq_ignoring_spans`, and spans too), and the gate passes.

## Answer

`#[derive(Clone)]` on `Document` and on every node type in `usfm_ast` (the
cursor types excepted: they borrow a tree rather than being one).
`usfm_codegen`'s `every_parse_clones_to_an_equal_tree` clones the parse of
every case of both conformance roots, `fail` included, and asserts that the
clone equals the original, spans and all, and shares its stylesheet `Arc`.

Michael, 2026-09-28, asked whether a full copy is necessary. It isn't. Both
of render's pipelines borrow the one edited `Document`, and each builds its
own model from it, so a clone is never required. The derive stays as a
convenience: it costs nothing until it is called, and a clone shares text
the parse borrowed from the source.
