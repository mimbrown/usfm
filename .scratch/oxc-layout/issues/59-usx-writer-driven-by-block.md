# 59. A public `UsxWriter`, driven block by block

Status: ready-for-agent
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
