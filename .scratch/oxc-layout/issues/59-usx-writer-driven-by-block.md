# 59. A public `UsxWriter`, driven block by block

Status: resolved (2026-09-28)
Milestone: after M7 (publishing foundation)

render writes its own XML for SILE. That XML is mostly our USX, with its
own elements between blocks: a cartouche around several paragraphs, a
bismillah, a title group. Today `usfm_usx` offers only whole-document
functions; its `UsxWriter` is private.

Make the writer public:

- `UsxWriter::new(sheet, options)`;
- `block(&Block)`;
- `open(name, attributes)`, `close()` and `empty(name, attributes)` for a
  caller's own elements;
- `finish() -> XmlNode`.

The book, the chapter, the open verse and `vid` carry over from one call
to the next, so a cartouche in the middle of a chapter changes nothing
the writer tracks. Add a hooks trait whose default methods write what the
writer writes today, so a caller can override one kind of node.

`to_usx_node` becomes a loop over `block`, and the conformance and USX
properties must stay byte-identical.

## Answer

`usfm_usx::UsxWriter` is public (ticket 59):
- `new(sheet, options)`, or `with_hooks(sheet, options, hooks)`;
- `document`, `block` and `inline`;
- `open` / `close` / `empty` / `text` / `node` for a caller's own elements;
- `finish() -> Vec<XmlNode>`, which closes whatever is still open.

The book, chapter and open verse carry across calls. `block` writes nothing
for the `\usfm` paragraph: USX spells it as the root's `version`, and a
caller writing its own root writes that too.

`UsxHooks` has one method per node kind. Each default calls the writer's
`write_*`. The methods are associated functions taking the writer, so a
hook can call back into it, and a hook's children go through the hooks
again. If a hook leaves an element it opened unclosed inside a node, that
panics with a message saying so.

`to_usx_node` is now `document` + `finish`, and the conformance and USX
properties are unchanged. Tests are in `crates/usfm_usx/tests/writer.rs`.
