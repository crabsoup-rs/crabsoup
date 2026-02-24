use anyhow::Result;
use tidier::{Doc, FormatOptions, Indent, LineEnding};
use tsugiki::dom::NodeRef;

pub mod extract_text;
pub mod is_document;

pub fn pretty_print(text: &str) -> Result<String> {
    let opts = FormatOptions {
        indent: Indent::DEFAULT,
        eol: LineEnding::Lf,
        wrap: 120,
        join_classes: true,
        join_styles: true,
        ..FormatOptions::DEFAULT
    };
    let doc = Doc::new(text, false)?;
    doc.repair()?;
    Ok(doc.format(&opts)?)
}

pub fn clone_node(node: &NodeRef) -> NodeRef {
    let data = node.0.data().clone();
    let new_node = NodeRef::new(data);

    for child in node.children() {
        new_node.append(clone_node(&child));
    }

    new_node
}
