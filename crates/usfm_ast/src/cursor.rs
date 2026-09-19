use crate::visit::Visit;
use crate::{
    Block, Book, ChapterEnd, ChapterStart, Char, Document, Inline, Milestone, Note, OptBreak, Para,
    Periph, Sidebar, Table, TableCell, TableRow, Text, VerseEnd, VerseStart,
};

/// The path to a node from the document root: the block index, then the
/// index of each child on the way down. Paths order like document order: a
/// parent sorts before its descendants, a node before its later siblings.
///
/// It is what [`NodeRef::descend`] takes and what [`NodeRef::descendants`]
/// builds, which is why it lives here rather than with the index that made it
/// (`usfm_semantic::ReferenceIndex`, ticket 22).
pub type NodePath = Vec<usize>;

/// A reference to any node in the AST tree.
#[derive(Debug, Clone, Copy)]
pub enum NodeRef<'a> {
    Document(&'a Document<'a>),
    Book(&'a Book<'a>),
    ChapterStart(&'a ChapterStart<'a>),
    ChapterEnd(&'a ChapterEnd),
    Para(&'a Para<'a>),
    Table(&'a Table<'a>),
    TableRow(&'a TableRow<'a>),
    TableCell(&'a TableCell<'a>),
    Text(&'a Text<'a>),
    VerseStart(&'a VerseStart<'a>),
    VerseEnd(&'a VerseEnd),
    Char(&'a Char<'a>),
    Note(&'a Note<'a>),
    Milestone(&'a Milestone<'a>),
    OptBreak(&'a OptBreak),
    Sidebar(&'a Sidebar<'a>),
    Periph(&'a Periph<'a>),
}

impl<'a> NodeRef<'a> {
    pub fn from_block(block: &'a Block<'a>) -> Self {
        match block {
            Block::Book(b) => NodeRef::Book(b),
            Block::ChapterStart(c) => NodeRef::ChapterStart(c),
            Block::ChapterEnd(c) => NodeRef::ChapterEnd(c),
            Block::Para(p) => NodeRef::Para(p),
            Block::Table(t) => NodeRef::Table(t),
            Block::Milestone(m) => NodeRef::Milestone(m),
            Block::Sidebar(s) => NodeRef::Sidebar(s),
            Block::Periph(p) => NodeRef::Periph(p),
        }
    }

    pub fn from_inline(inline: &'a Inline<'a>) -> Self {
        match inline {
            Inline::Text(t) => NodeRef::Text(t),
            Inline::VerseStart(v) => NodeRef::VerseStart(v),
            Inline::VerseEnd(v) => NodeRef::VerseEnd(v),
            Inline::Char(c) => NodeRef::Char(c),
            Inline::Note(n) => NodeRef::Note(n),
            Inline::Milestone(m) => NodeRef::Milestone(m),
            Inline::OptBreak(b) => NodeRef::OptBreak(b),
        }
    }

    /// Returns the number of children this node has.
    pub fn num_children(&self) -> usize {
        match self {
            NodeRef::Document(d) => d.blocks.len(),
            NodeRef::Para(p) => p.children.len(),
            NodeRef::Char(c) => c.children.len(),
            NodeRef::Note(n) => n.children.len(),
            NodeRef::Table(t) => t.rows.len(),
            NodeRef::TableRow(r) => r.cells.len(),
            NodeRef::TableCell(c) => c.children.len(),
            NodeRef::Sidebar(s) => s.blocks.len(),
            NodeRef::Periph(p) => p.blocks.len(),
            // Leaf nodes
            NodeRef::Book(_)
            | NodeRef::ChapterStart(_)
            | NodeRef::ChapterEnd(_)
            | NodeRef::Text(_)
            | NodeRef::VerseStart(_)
            | NodeRef::VerseEnd(_)
            | NodeRef::Milestone(_)
            | NodeRef::OptBreak(_) => 0,
        }
    }

    pub fn is_leaf(&self) -> bool {
        self.num_children() == 0
    }

    /// The `index`th child; `index` must be in range.
    fn nth_child(&self, index: usize) -> NodeRef<'a> {
        match *self {
            NodeRef::Document(d) => NodeRef::from_block(&d.blocks[index]),
            NodeRef::Para(p) => NodeRef::from_inline(&p.children[index]),
            NodeRef::Char(c) => NodeRef::from_inline(&c.children[index]),
            NodeRef::Note(n) => NodeRef::from_inline(&n.children[index]),
            NodeRef::Table(t) => NodeRef::TableRow(&t.rows[index]),
            NodeRef::TableRow(r) => NodeRef::TableCell(&r.cells[index]),
            NodeRef::TableCell(c) => NodeRef::from_inline(&c.children[index]),
            NodeRef::Sidebar(s) => NodeRef::from_block(&s.blocks[index]),
            NodeRef::Periph(p) => NodeRef::from_block(&p.blocks[index]),
            _ => unreachable!("nth_child called on a leaf node"),
        }
    }

    /// The `index`th child, or `None` past the end or on a leaf.
    pub fn child(&self, index: usize) -> Option<NodeRef<'a>> {
        (index < self.num_children()).then(|| self.nth_child(index))
    }

    /// The node at `path` below this one: each element is a child index,
    /// so an empty path is this node.
    pub fn descend(&self, path: &[usize]) -> Option<NodeRef<'a>> {
        path.iter()
            .try_fold(*self, |node, &index| node.child(index))
    }

    /// Every descendant in document order (pre-order), with its path from
    /// this node. `path` is the prefix to start from, normally empty; it is
    /// borrowed for the walk and left as it was.
    pub fn descendants(&self, path: &mut Vec<usize>, f: &mut impl FnMut(&[usize], NodeRef<'a>)) {
        for index in 0..self.num_children() {
            let child = self.nth_child(index);
            path.push(index);
            f(path, child);
            child.descendants(path, f);
            path.pop();
        }
    }

    /// Visit this node with `visitor`, dispatching on its kind.
    pub fn visit_with<V: Visit>(&self, visitor: &mut V) {
        match *self {
            NodeRef::Document(d) => visitor.visit_document(d),
            NodeRef::Book(b) => visitor.visit_book(b),
            NodeRef::ChapterStart(c) => visitor.visit_chapter_start(c),
            NodeRef::ChapterEnd(c) => visitor.visit_chapter_end(c),
            NodeRef::Para(p) => visitor.visit_para(p),
            NodeRef::Table(t) => visitor.visit_table(t),
            NodeRef::TableRow(r) => visitor.visit_table_row(r),
            NodeRef::TableCell(c) => visitor.visit_table_cell(c),
            NodeRef::Text(t) => visitor.visit_text(t),
            NodeRef::VerseStart(v) => visitor.visit_verse_start(v),
            NodeRef::VerseEnd(v) => visitor.visit_verse_end(v),
            NodeRef::Char(c) => visitor.visit_char(c),
            NodeRef::Note(n) => visitor.visit_note(n),
            NodeRef::Milestone(m) => visitor.visit_milestone(m),
            NodeRef::OptBreak(b) => visitor.visit_opt_break(b),
            NodeRef::Sidebar(s) => visitor.visit_sidebar(s),
            NodeRef::Periph(p) => visitor.visit_periph(p),
        }
    }

    pub fn is_document(&self) -> bool {
        matches!(self, NodeRef::Document(_))
    }
    pub fn is_book(&self) -> bool {
        matches!(self, NodeRef::Book(_))
    }
    pub fn is_chapter_start(&self) -> bool {
        matches!(self, NodeRef::ChapterStart(_))
    }
    pub fn is_chapter_end(&self) -> bool {
        matches!(self, NodeRef::ChapterEnd(_))
    }
    pub fn is_para(&self) -> bool {
        matches!(self, NodeRef::Para(_))
    }
    pub fn is_table(&self) -> bool {
        matches!(self, NodeRef::Table(_))
    }
    pub fn is_table_row(&self) -> bool {
        matches!(self, NodeRef::TableRow(_))
    }
    pub fn is_table_cell(&self) -> bool {
        matches!(self, NodeRef::TableCell(_))
    }
    pub fn is_text(&self) -> bool {
        matches!(self, NodeRef::Text(_))
    }
    pub fn is_verse_start(&self) -> bool {
        matches!(self, NodeRef::VerseStart(_))
    }
    pub fn is_verse_end(&self) -> bool {
        matches!(self, NodeRef::VerseEnd(_))
    }
    pub fn is_char(&self) -> bool {
        matches!(self, NodeRef::Char(_))
    }
    pub fn is_note(&self) -> bool {
        matches!(self, NodeRef::Note(_))
    }
    pub fn is_milestone(&self) -> bool {
        matches!(self, NodeRef::Milestone(_))
    }
    pub fn is_sidebar(&self) -> bool {
        matches!(self, NodeRef::Sidebar(_))
    }
    pub fn is_periph(&self) -> bool {
        matches!(self, NodeRef::Periph(_))
    }

    pub fn as_document(&self) -> Option<&'a Document<'a>> {
        match self {
            NodeRef::Document(d) => Some(d),
            _ => None,
        }
    }
    pub fn as_book(&self) -> Option<&'a Book<'a>> {
        match self {
            NodeRef::Book(b) => Some(b),
            _ => None,
        }
    }
    pub fn as_chapter_start(&self) -> Option<&'a ChapterStart<'a>> {
        match self {
            NodeRef::ChapterStart(c) => Some(c),
            _ => None,
        }
    }
    pub fn as_chapter_end(&self) -> Option<&'a ChapterEnd> {
        match self {
            NodeRef::ChapterEnd(c) => Some(c),
            _ => None,
        }
    }
    pub fn as_para(&self) -> Option<&'a Para<'a>> {
        match self {
            NodeRef::Para(p) => Some(p),
            _ => None,
        }
    }
    pub fn as_table(&self) -> Option<&'a Table<'a>> {
        match self {
            NodeRef::Table(t) => Some(t),
            _ => None,
        }
    }
    pub fn as_table_row(&self) -> Option<&'a TableRow<'a>> {
        match self {
            NodeRef::TableRow(r) => Some(r),
            _ => None,
        }
    }
    pub fn as_table_cell(&self) -> Option<&'a TableCell<'a>> {
        match self {
            NodeRef::TableCell(c) => Some(c),
            _ => None,
        }
    }
    pub fn as_text(&self) -> Option<&'a Text<'a>> {
        match self {
            NodeRef::Text(t) => Some(t),
            _ => None,
        }
    }
    pub fn as_verse_start(&self) -> Option<&'a VerseStart<'a>> {
        match self {
            NodeRef::VerseStart(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_verse_end(&self) -> Option<&'a VerseEnd> {
        match self {
            NodeRef::VerseEnd(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_char(&self) -> Option<&'a Char<'a>> {
        match self {
            NodeRef::Char(c) => Some(c),
            _ => None,
        }
    }
    pub fn as_note(&self) -> Option<&'a Note<'a>> {
        match self {
            NodeRef::Note(n) => Some(n),
            _ => None,
        }
    }
    pub fn as_milestone(&self) -> Option<&'a Milestone<'a>> {
        match self {
            NodeRef::Milestone(m) => Some(m),
            _ => None,
        }
    }
    pub fn as_sidebar(&self) -> Option<&'a Sidebar<'a>> {
        match self {
            NodeRef::Sidebar(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_periph(&self) -> Option<&'a Periph<'a>> {
        match self {
            NodeRef::Periph(p) => Some(p),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct Frame<'a> {
    node: NodeRef<'a>,
    child_index: usize,
}

/// A read-only cursor (zipper) for navigating the AST in any direction.
///
/// The cursor borrows the document immutably and maintains a stack of frames
/// representing the path from the root to the current node. Navigation methods
/// return `bool` — `true` if the move succeeded, `false` if it couldn't move.
#[derive(Debug)]
pub struct Cursor<'a> {
    stack: Vec<Frame<'a>>,
}

impl<'a> Cursor<'a> {
    /// Create a new cursor positioned at the document root.
    pub fn new(document: &'a Document<'a>) -> Self {
        Cursor {
            stack: vec![Frame {
                node: NodeRef::Document(document),
                child_index: 0,
            }],
        }
    }

    /// Returns the current node.
    pub fn node(&self) -> NodeRef<'a> {
        self.stack.last().unwrap().node
    }

    /// Returns the parent of the current node, if any.
    pub fn parent_node(&self) -> Option<NodeRef<'a>> {
        if self.stack.len() >= 2 {
            Some(self.stack[self.stack.len() - 2].node)
        } else {
            None
        }
    }

    /// Returns the depth of the current node (0 at the root).
    pub fn depth(&self) -> usize {
        self.stack.len() - 1
    }

    /// Move to the first child of the current node. Returns `false` if the
    /// current node has no children.
    pub fn first_child(&mut self) -> bool {
        let current = self.stack.last().unwrap().node;
        if current.num_children() == 0 {
            return false;
        }
        let child = Self::nth_child_of(current, 0);
        self.stack.last_mut().unwrap().child_index = 0;
        self.stack.push(Frame {
            node: child,
            child_index: 0,
        });
        true
    }

    /// Move to the last child of the current node. Returns `false` if the
    /// current node has no children.
    pub fn last_child(&mut self) -> bool {
        let current = self.stack.last().unwrap().node;
        let count = current.num_children();
        if count == 0 {
            return false;
        }
        let last = count - 1;
        let child = Self::nth_child_of(current, last);
        self.stack.last_mut().unwrap().child_index = last;
        self.stack.push(Frame {
            node: child,
            child_index: 0,
        });
        true
    }

    /// Move to the next sibling of the current node. Returns `false` if the
    /// current node is the root or the last child of its parent.
    pub fn next_sibling(&mut self) -> bool {
        if self.stack.len() < 2 {
            return false;
        }
        let parent_frame = &self.stack[self.stack.len() - 2];
        let parent = parent_frame.node;
        let current_index = parent_frame.child_index;
        let next_index = current_index + 1;
        if next_index >= parent.num_children() {
            return false;
        }
        let sibling = Self::nth_child_of(parent, next_index);
        self.stack.pop();
        self.stack.last_mut().unwrap().child_index = next_index;
        self.stack.push(Frame {
            node: sibling,
            child_index: 0,
        });
        true
    }

    /// Move to the previous sibling of the current node. Returns `false` if the
    /// current node is the root or the first child of its parent.
    pub fn prev_sibling(&mut self) -> bool {
        if self.stack.len() < 2 {
            return false;
        }
        let parent_frame = &self.stack[self.stack.len() - 2];
        let parent = parent_frame.node;
        let current_index = parent_frame.child_index;
        if current_index == 0 {
            return false;
        }
        let prev_index = current_index - 1;
        let sibling = Self::nth_child_of(parent, prev_index);
        self.stack.pop();
        self.stack.last_mut().unwrap().child_index = prev_index;
        self.stack.push(Frame {
            node: sibling,
            child_index: 0,
        });
        true
    }

    /// Move to the parent of the current node. Returns `false` if the current
    /// node is the root.
    pub fn parent(&mut self) -> bool {
        if self.stack.len() < 2 {
            return false;
        }
        self.stack.pop();
        true
    }

    /// Reset the cursor to the document root.
    pub fn reset(&mut self) {
        self.stack.truncate(1);
    }

    fn nth_child_of(parent: NodeRef<'a>, index: usize) -> NodeRef<'a> {
        parent.nth_child(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    fn sample_document<'a>() -> Document<'a> {
        Document::without_styles(vec![
            Block::Book(Book {
                span: SPAN,
                code: BookCode::Gen,
                description: Cow::Borrowed("Genesis"),
            }),
            Block::ChapterStart(ChapterStart {
                span: SPAN,
                number: 1,
                alt_number: None,
                pub_number: None,
            }),
            Block::Para(Para {
                span: SPAN,
                style: StyleId::new(0),
                children: vec![
                    Inline::VerseStart(VerseStart {
                        span: SPAN,
                        number: NumberList::collapsed(1),
                        alt_number: None,
                        pub_number: None,
                    }),
                    Inline::Text(Text::synthesized("In the beginning")),
                    Inline::Char(Char {
                        span: SPAN,
                        style: StyleId::new(0),
                        children: vec![Inline::Text(Text::synthesized("God"))],
                        attributes: None,
                    }),
                ],
            }),
        ])
    }

    #[test]
    fn navigate_down_and_across_siblings() {
        let doc = sample_document();
        let mut cursor = Cursor::new(&doc);

        assert!(cursor.node().is_document());
        assert_eq!(cursor.depth(), 0);

        // Down to first child (Book)
        assert!(cursor.first_child());
        assert!(cursor.node().is_book());
        assert_eq!(cursor.depth(), 1);

        // Next sibling (ChapterStart)
        assert!(cursor.next_sibling());
        assert!(cursor.node().is_chapter_start());

        // Next sibling (Para)
        assert!(cursor.next_sibling());
        assert!(cursor.node().is_para());

        // No more siblings
        assert!(!cursor.next_sibling());
    }

    #[test]
    fn navigate_into_para_children() {
        let doc = sample_document();
        let mut cursor = Cursor::new(&doc);

        // Navigate to Para: doc -> last child
        assert!(cursor.last_child());
        assert!(cursor.node().is_para());

        // Into Para's children
        assert!(cursor.first_child());
        assert!(cursor.node().is_verse_start());

        assert!(cursor.next_sibling());
        assert!(cursor.node().is_text());
        assert_eq!(
            cursor.node().as_text().unwrap().content.as_ref(),
            "In the beginning"
        );

        assert!(cursor.next_sibling());
        assert!(cursor.node().is_char());
    }

    #[test]
    fn navigate_into_char_children() {
        let doc = sample_document();
        let mut cursor = Cursor::new(&doc);

        // doc -> Para -> Char -> Text
        cursor.last_child(); // Para
        cursor.last_child(); // Char
        assert!(cursor.node().is_char());

        assert!(cursor.first_child());
        assert!(cursor.node().is_text());
        assert_eq!(cursor.node().as_text().unwrap().content.as_ref(), "God");

        // Leaf: can't go deeper
        assert!(!cursor.first_child());
        assert!(!cursor.last_child());
    }

    #[test]
    fn navigate_up_to_root() {
        let doc = sample_document();
        let mut cursor = Cursor::new(&doc);

        // Go deep: doc -> Para -> Char -> Text
        cursor.last_child();
        cursor.last_child();
        cursor.first_child();
        assert!(cursor.node().is_text());
        assert_eq!(cursor.depth(), 3);

        // Walk back up
        assert!(cursor.parent());
        assert!(cursor.node().is_char());
        assert_eq!(cursor.depth(), 2);

        assert!(cursor.parent());
        assert!(cursor.node().is_para());
        assert_eq!(cursor.depth(), 1);

        assert!(cursor.parent());
        assert!(cursor.node().is_document());
        assert_eq!(cursor.depth(), 0);

        // Can't go above root
        assert!(!cursor.parent());
    }

    #[test]
    fn prev_sibling() {
        let doc = sample_document();
        let mut cursor = Cursor::new(&doc);

        // Go to last block
        cursor.last_child();
        assert!(cursor.node().is_para());

        // Walk backwards
        assert!(cursor.prev_sibling());
        assert!(cursor.node().is_chapter_start());

        assert!(cursor.prev_sibling());
        assert!(cursor.node().is_book());

        // Can't go before first
        assert!(!cursor.prev_sibling());
    }

    #[test]
    fn root_boundary_checks() {
        let doc = sample_document();
        let mut cursor = Cursor::new(&doc);

        assert!(!cursor.parent());
        assert!(!cursor.next_sibling());
        assert!(!cursor.prev_sibling());
        assert!(cursor.parent_node().is_none());
    }

    #[test]
    fn leaf_boundary_checks() {
        let doc = sample_document();
        let mut cursor = Cursor::new(&doc);

        // Navigate to a leaf (Book)
        cursor.first_child();
        assert!(cursor.node().is_book());
        assert!(cursor.node().is_leaf());
        assert!(!cursor.first_child());
        assert!(!cursor.last_child());
    }

    #[test]
    fn parent_node() {
        let doc = sample_document();
        let mut cursor = Cursor::new(&doc);

        cursor.last_child(); // Para
        cursor.first_child(); // VerseStart

        let parent = cursor.parent_node().unwrap();
        assert!(parent.is_para());
    }

    #[test]
    fn reset_returns_to_root() {
        let doc = sample_document();
        let mut cursor = Cursor::new(&doc);

        cursor.last_child();
        cursor.last_child();
        cursor.first_child();
        assert_eq!(cursor.depth(), 3);

        cursor.reset();
        assert_eq!(cursor.depth(), 0);
        assert!(cursor.node().is_document());
    }

    #[test]
    fn num_children_correctness() {
        let doc = sample_document();
        let cursor = Cursor::new(&doc);

        // Document has 3 blocks
        assert_eq!(cursor.node().num_children(), 3);
    }

    #[test]
    fn table_traversal() {
        let doc = Document::without_styles(vec![Block::Table(Table {
            span: SPAN,
            rows: vec![TableRow {
                span: SPAN,
                cells: vec![TableCell {
                    span: SPAN,
                    header: false,
                    alignment: Alignment::Start,
                    column: 1,
                    colspan: 1,
                    children: vec![Inline::Text(Text::synthesized("cell content"))],
                }],
            }],
        })]);

        let mut cursor = Cursor::new(&doc);

        assert!(cursor.first_child());
        assert!(cursor.node().is_table());

        assert!(cursor.first_child());
        assert!(cursor.node().is_table_row());

        assert!(cursor.first_child());
        assert!(cursor.node().is_table_cell());

        assert!(cursor.first_child());
        assert!(cursor.node().is_text());
        assert_eq!(
            cursor.node().as_text().unwrap().content.as_ref(),
            "cell content"
        );

        // Walk back up
        assert!(cursor.parent());
        assert!(cursor.node().is_table_cell());
        assert!(cursor.parent());
        assert!(cursor.node().is_table_row());
        assert!(cursor.parent());
        assert!(cursor.node().is_table());
        assert!(cursor.parent());
        assert!(cursor.node().is_document());
    }

    #[test]
    fn note_traversal() {
        let doc = Document::without_styles(vec![Block::Para(Para {
            span: SPAN,
            style: StyleId::new(0),
            children: vec![Inline::Note(Note {
                span: SPAN,
                style: StyleId::new(0),
                caller: Caller::Plus,
                category: None,
                children: vec![Inline::Text(Text::synthesized("footnote text"))],
            })],
        })]);

        let mut cursor = Cursor::new(&doc);

        cursor.first_child(); // Para
        cursor.first_child(); // Note
        assert!(cursor.node().is_note());
        assert_eq!(cursor.node().num_children(), 1);

        cursor.first_child(); // Text inside note
        assert!(cursor.node().is_text());
        assert_eq!(
            cursor.node().as_text().unwrap().content.as_ref(),
            "footnote text"
        );
    }

    #[test]
    fn empty_document() {
        let doc = Document::without_styles(vec![]);
        let mut cursor = Cursor::new(&doc);

        assert!(cursor.node().is_document());
        assert_eq!(cursor.node().num_children(), 0);
        assert!(!cursor.first_child());
        assert!(!cursor.last_child());
    }
}
