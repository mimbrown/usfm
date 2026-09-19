//! SILE output.
//!
//! SILE reads the USX tree under a `<sile>` root with no attributes, so this
//! is the USX writer's tree with its root renamed — it has been that since
//! ticket 06, and the generic `Serialize` trait the first version was written
//! against had no implementor left by ticket 15, which deleted it.

use usfm_ast::Document;
use usfm_usx::{XmlNode, to_usx_node};

/// `document` as SILE's flavour of USX: the USX tree under a `<sile>` root
/// with no attributes, and a trailing newline.
pub fn to_sile_string(document: &Document) -> String {
    let mut node = to_usx_node(document);
    if let XmlNode::Element(root) = &mut node {
        root.name.local_name = "sile".to_string();
        root.attributes.clear();
    }
    format!("{node}\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::parse;

    /// The root is `<sile>` and carries none of USX's attributes; everything
    /// under it is the USX tree, so the book and its verses are still there.
    #[test]
    fn the_root_is_a_bare_sile_element() {
        let (document, _) = parse("\\id GEN\n\\c 1\n\\p \\v 1 verse one");
        let sile = to_sile_string(&document);

        assert!(sile.starts_with("<sile>"), "{sile}");
        assert!(sile.ends_with("</sile>\n"), "{sile}");
        assert!(!sile.contains("version="), "{sile}");
        assert!(sile.contains("<book code=\"GEN\""), "{sile}");
        assert!(sile.contains("verse one"), "{sile}");
    }
}
