use crate::{
    html::{
        clone_node,
        extract_text::{inner_text, strip_tags},
        is_document::is_document,
    },
    wyhash::WyHashSet,
};
use encoding_rs::{Encoding, UTF_8};
use html5ever::{namespace_url, ns, LocalName, QualName};
use kuchikiki::{
    parse_fragment, parse_html, traits::TendrilSink, ElementData, NodeDataRef, NodeRef, Selectors,
};
use mlua::{prelude::LuaString, Error, Lua, Result, Table, UserData, UserDataFields, UserDataRef};
use std::{borrow::Cow, cell::RefCell, io::Cursor, ops::Deref, rc::Rc, str::Split};
use tracing::warn;

fn qual_name(name: &str) -> QualName {
    QualName { prefix: None, ns: ns!(html), local: LocalName::from(name) }
}
fn decode_encoding(encoding: &LuaString) -> Result<&'static Encoding> {
    match Encoding::for_label(&encoding.as_bytes()) {
        None => {
            Err(Error::runtime(format_args!("Unknown encoding: {}", encoding.to_string_lossy())))
        }
        Some(encoding) => Ok(encoding),
    }
}
fn encoding_warning(lua: &Lua, encoding: &'static Encoding, encode: bool) {
    let location = if let Some(source) = lua.inspect_stack(1, |debug| {
        debug
            .source()
            .source
            .map(|x| x.to_string())
            .unwrap_or_else(|| "<unknown>".to_string())
    }) {
        source
    } else {
        "<unknown>".to_string()
    };
    if encode {
        warn!("{location}: Encountered invalid {} while serializing HTML.", encoding.name());
    } else {
        warn!("{location}: Encountered invalid {} while parsing HTML.", encoding.name());
    }
}
fn html_to_string<'lua>(
    lua: &'lua Lua,
    node: &NodeRef,
    encoding: &Option<LuaString>,
    active_encoding_ref: &Rc<RefCell<&'static Encoding>>,
    pretty_print: bool,
) -> Result<LuaString> {
    let encoding = match encoding {
        None => *active_encoding_ref.borrow(),
        Some(encoding) => decode_encoding(&encoding)?,
    };

    let mut data = Vec::new();
    node.serialize(&mut Cursor::new(&mut data))?;
    let rust_str = std::str::from_utf8(&data)?;
    let processed = if pretty_print {
        match crate::html::pretty_print(rust_str) {
            Ok(value) => Cow::Owned(value),
            Err(_) => {
                warn!("Failed to pretty print HTML document.");
                Cow::Borrowed(rust_str)
            }
        }
    } else {
        Cow::Borrowed(rust_str)
    };
    let (text, encoding, errors) = encoding.encode(&processed);
    if errors {
        encoding_warning(lua, encoding, true);
    }
    Ok(lua.create_string(&text)?)
}
fn element(node: &NodeRef) -> Result<NodeDataRef<ElementData>> {
    match node.clone().into_element_ref() {
        None => Err(Error::runtime("This function may only be called on HTML elements.")),
        Some(elem) => Ok(elem),
    }
}

static SELECTOR_WHITESPACE: &[char] = &[' ', '\t', '\n', '\r', '\x0C'];
fn split_classes(input: &str) -> impl Iterator<Item = &str> {
    struct SplitClasses<'a> {
        underlying: Split<'a, &'static [char]>,
        found: WyHashSet<&'a str>,
    }
    impl<'a> Iterator for SplitClasses<'a> {
        type Item = &'a str;
        fn next(&mut self) -> Option<Self::Item> {
            if let Some(next) = self.underlying.next() {
                let next = next.trim();
                if !next.is_empty() && self.found.insert(next) {
                    Some(next)
                } else {
                    self.next()
                }
            } else {
                None
            }
        }
    }

    SplitClasses { underlying: input.split(SELECTOR_WHITESPACE), found: Default::default() }
}
fn check_class_name(name: &str) -> Result<()> {
    if name.contains(SELECTOR_WHITESPACE) || name.trim() != name {
        Err(Error::runtime("Invalid class name."))
    } else {
        Ok(())
    }
}

fn parse<'lua>(
    lua: &'lua Lua,
    text: LuaString,
    encoding: Option<LuaString>,
    force_document: bool,
    force_fragment: bool,
    fragment_root: &str,
    active_encoding_ref: &Rc<RefCell<&'static Encoding>>,
) -> Result<LuaNodeRef> {
    assert!(!(force_document && force_fragment));

    let encoding = match encoding {
        None => *active_encoding_ref.borrow(),
        Some(encoding) => decode_encoding(&encoding)?,
    };
    let text = text.as_bytes();
    let (text, encoding, errors) = encoding.decode(&text);
    if errors {
        encoding_warning(lua, encoding, false);
    }
    if (force_document || is_document(&text)) && !force_fragment {
        Ok(LuaNodeRef(parse_html().one(&*text)))
    } else {
        let fragment = parse_fragment(qual_name(fragment_root), vec![]).one(&*text);
        let new_root = NodeRef::new_document();
        assert_eq!(fragment.children().count(), 1);
        for child in fragment.children().next().unwrap().children() {
            new_root.append(child);
        }
        Ok(LuaNodeRef(new_root))
    }
}

pub fn create_html_table(lua: &Lua) -> Result<Table> {
    let table = lua.create_table()?;

    let active_encoding = Rc::new(RefCell::new(UTF_8));

    // Parsing and rendering
    {
        let active_encoding_ref = active_encoding.clone();
        table.raw_set(
            "parse",
            lua.create_function(move |lua, (text, encoding): (LuaString, Option<LuaString>)| {
                parse(lua, text, encoding, false, false, "main", &active_encoding_ref)
            })?,
        )?;
    }
    {
        let active_encoding_ref = active_encoding.clone();
        table.raw_set(
            "parse_document",
            lua.create_function(move |lua, (text, encoding): (LuaString, Option<LuaString>)| {
                parse(lua, text, encoding, true, false, "main", &active_encoding_ref)
            })?,
        )?;
    }
    {
        let active_encoding_ref = active_encoding.clone();
        table.raw_set(
            "parse_fragment",
            lua.create_function(
                move |lua, (text, tag, encoding): (LuaString, LuaString, Option<LuaString>)| {
                    parse(lua, text, encoding, false, true, &tag.to_str()?, &active_encoding_ref)
                },
            )?,
        )?;
    }
    {
        let active_encoding_ref = active_encoding.clone();
        table.raw_set(
            "set_default_encoding",
            lua.create_function(move |_, encoding: LuaString| {
                *active_encoding_ref.borrow_mut() = decode_encoding(&encoding)?;
                Ok(())
            })?,
        )?;
    }
    {
        let active_encoding_ref = active_encoding.clone();
        table.raw_set(
            "to_string",
            lua.create_function(
                move |lua, (node_ref, encoding): (UserDataRef<LuaNodeRef>, Option<LuaString>)| {
                    html_to_string(lua, &node_ref.0, &encoding, &active_encoding_ref, false)
                },
            )?,
        )?;
    }
    {
        let active_encoding_ref = active_encoding.clone();
        table.raw_set(
            "pretty_print",
            lua.create_function(
                move |lua, (node_ref, encoding): (UserDataRef<LuaNodeRef>, Option<LuaString>)| {
                    html_to_string(lua, &node_ref.0, &encoding, &active_encoding_ref, true)
                },
            )?,
        )?;
    }
    {
        let active_encoding_ref = active_encoding.clone();
        table.raw_set(
            "inner_html",
            lua.create_function(
                move |lua, (node_ref, encoding): (UserDataRef<LuaNodeRef>, Option<LuaString>)| {
                    let encoding = match encoding {
                        None => *active_encoding_ref.borrow(),
                        Some(encoding) => decode_encoding(&encoding)?,
                    };

                    let mut data = Vec::new();
                    let mut cursor = Cursor::new(&mut data);
                    for child in node_ref.0.children() {
                        child.serialize(&mut cursor)?;
                    }
                    let rust_str = std::str::from_utf8(&data)?;

                    let (text, encoding, errors) = encoding.encode(&rust_str);
                    if errors {
                        encoding_warning(lua, encoding, true);
                    }
                    Ok(lua.create_string(&text)?)
                },
            )?,
        )?;
    }

    // Node creation
    table.raw_set(
        "create_document",
        lua.create_function(|_, ()| Ok(LuaNodeRef(NodeRef::new_document())))?,
    )?;
    table.raw_set(
        "create_element",
        lua.create_function(|_, (name, text): (LuaString, Option<LuaString>)| {
            let elem = NodeRef::new_element(qual_name(&name.to_str()?), vec![]);
            if let Some(text) = text {
                elem.append(NodeRef::new_text(text.to_str()?.to_string()));
            }
            Ok(LuaNodeRef(elem))
        })?,
    )?;
    table.raw_set(
        "create_text",
        lua.create_function(|_, text: LuaString| {
            Ok(LuaNodeRef(NodeRef::new_text(text.to_str()?.to_string())))
        })?,
    )?;
    table.raw_set(
        "clone",
        lua.create_function(|_, elem: UserDataRef<LuaNodeRef>| {
            Ok(LuaNodeRef(clone_node(&elem.0)))
        })?,
    )?;

    // Selection and selector match checking
    // - Implemented in Lua: HTML.select_any_of - note: crabsoup uses selector lists
    // - Implemented in Lua: HTML.select_all_of - note: crabsoup uses selector lists
    // - Implemented in Lua: HTML.matches_selector - note: crabsoup doesn't need the document ref
    // - Implemented in Lua: HTML.matches_any_of_selectors - note: crabsoup uses selector lists
    table.raw_set(
        "select",
        lua.create_function(|lua, (html, selector): (UserDataRef<LuaNodeRef>, LuaString)| {
            let selector = selector.to_str()?;
            let table = lua.create_table()?;
            for elem in html.0.select(&selector).map_err(|()| {
                Error::runtime(format_args!("Could not parse selector: {selector}"))
            })? {
                table.raw_push(LuaNodeRef(elem.as_node().clone()))?;
            }
            Ok(table)
        })?,
    )?;
    table.raw_set(
        "select_one",
        lua.create_function(|_, (html, selector): (UserDataRef<LuaNodeRef>, LuaString)| {
            let selector = selector.to_str()?;
            Ok(html
                .0
                .select(&selector)
                .map_err(|()| Error::runtime(format_args!("Could not parse selector: {selector}")))?
                .next()
                .map(|x| LuaNodeRef(x.as_node().clone())))
        })?,
    )?;
    table.raw_set(
        "matches",
        lua.create_function(|_, (node, selector): (UserDataRef<LuaNodeRef>, LuaString)| {
            let selector = selector.to_str()?;
            let selectors = Selectors::compile(&selector).map_err(|()| {
                Error::runtime(format_args!("Could not parse selector: {selector}"))
            })?;
            if let Some(elem) = node.0.clone().into_element_ref() {
                Ok(selectors.matches(&elem))
            } else {
                Ok(false)
            }
        })?,
    )?;

    // Access to element tree surroundings
    table.raw_set(
        "parent",
        lua.create_function(|_, node: UserDataRef<LuaNodeRef>| {
            Ok(node.0.parent().map(LuaNodeRef))
        })?,
    )?;
    table.raw_set(
        "children",
        lua.create_function(|lua, node: UserDataRef<LuaNodeRef>| {
            let table = lua.create_table()?;
            for node in node.0.children() {
                table.raw_push(LuaNodeRef(node))?;
            }
            Ok(table)
        })?,
    )?;
    table.raw_set(
        "ancestors",
        lua.create_function(|lua, node: UserDataRef<LuaNodeRef>| {
            let table = lua.create_table()?;
            for node in node.0.ancestors() {
                table.raw_push(LuaNodeRef(node))?;
            }
            Ok(table)
        })?,
    )?;
    table.raw_set(
        "descendants",
        lua.create_function(|lua, node: UserDataRef<LuaNodeRef>| {
            let table = lua.create_table()?;
            for node in node.0.descendants() {
                table.raw_push(LuaNodeRef(node))?;
            }
            Ok(table)
        })?,
    )?;
    table.raw_set(
        "siblings",
        lua.create_function(|lua, root: UserDataRef<LuaNodeRef>| {
            if let Some(parent) = root.0.parent() {
                let table = lua.create_table()?;
                for node in parent.children() {
                    if &node != &root.0 {
                        table.raw_push(LuaNodeRef(node))?;
                    }
                }
                Ok(table)
            } else {
                Err(Error::runtime("Cannot call `siblings` on node without a parent."))
            }
        })?,
    )?;
    table.raw_set(
        "child_count",
        lua.create_function(|_, node: UserDataRef<LuaNodeRef>| Ok(node.0.children().count()))?,
    )?;
    table.raw_set(
        "is_empty",
        lua.create_function(|_, node: UserDataRef<LuaNodeRef>| {
            Ok(node.0.children().next().is_none())
        })?,
    )?;

    // Element property access and manipulation
    // - Implemented in Lua: HTML.append_attribute
    table.raw_set(
        "get_tag_name",
        lua.create_function(|_, node: UserDataRef<LuaNodeRef>| {
            Ok(element(&node.0)?.name.borrow().local.to_string())
        })?,
    )?;
    table.raw_set(
        "set_tag_name",
        lua.create_function(|_, (node, name): (UserDataRef<LuaNodeRef>, LuaString)| {
            element(&node.0)?.name.borrow_mut().local = LocalName::from(name.to_str()?.deref());
            Ok(())
        })?,
    )?;
    table.raw_set(
        "get_attribute",
        lua.create_function(|lua, (node, name): (UserDataRef<LuaNodeRef>, LuaString)| {
            if let Some(attr) = element(&node.0)?
                .attributes
                .borrow()
                .get(name.to_str()?.deref())
            {
                Ok(Some(lua.create_string(attr)?))
            } else {
                Ok(None)
            }
        })?,
    )?;
    table.raw_set(
        "set_attribute",
        lua.create_function(
            |_, (node, name, value): (UserDataRef<LuaNodeRef>, LuaString, LuaString)| {
                element(&node.0)?
                    .attributes
                    .borrow_mut()
                    .insert(name.to_str()?.deref(), value.to_string_lossy().to_string());
                Ok(())
            },
        )?,
    )?;
    table.raw_set(
        "delete_attribute",
        lua.create_function(|_, (node, name): (UserDataRef<LuaNodeRef>, LuaString)| {
            element(&node.0)?
                .attributes
                .borrow_mut()
                .remove(name.to_str()?.deref());
            Ok(())
        })?,
    )?;
    table.raw_set(
        "list_attributes",
        lua.create_function(|lua, node: UserDataRef<LuaNodeRef>| {
            let table = lua.create_table()?;
            for (attr, _) in element(&node.0)?.attributes.borrow().map.iter() {
                table.raw_push(lua.create_string(attr.local.to_string())?)?;
            }
            Ok(table)
        })?,
    )?;
    table.raw_set(
        "clear_attributes",
        lua.create_function(|_, node: UserDataRef<LuaNodeRef>| {
            element(&node.0)?.attributes.borrow_mut().map.clear();
            Ok(())
        })?,
    )?;
    table.raw_set(
        "get_classes",
        lua.create_function(|lua, node: UserDataRef<LuaNodeRef>| {
            let elem = element(&node.0)?;
            let attrs = elem.attributes.borrow();
            let classes = attrs.get("class");
            if let Some(classes) = classes {
                let table = lua.create_table()?;
                for class in split_classes(classes) {
                    table.raw_push(lua.create_string(class)?)?;
                }
                Ok(table)
            } else {
                Ok(lua.create_table()?)
            }
        })?,
    )?;
    table.raw_set(
        "has_class",
        lua.create_function(|_, (node, name): (UserDataRef<LuaNodeRef>, LuaString)| {
            let elem = element(&node.0)?;
            let attrs = elem.attributes.borrow();
            let classes = attrs.get("class");
            if let Some(classes) = classes {
                let name = name.to_str()?;
                check_class_name(&name)?;
                Ok(split_classes(classes).any(|x| x == name.deref()))
            } else {
                Ok(false)
            }
        })?,
    )?;
    table.raw_set(
        "add_class",
        lua.create_function(|_, (node, name): (UserDataRef<LuaNodeRef>, LuaString)| {
            let elem = element(&node.0)?;
            let mut attrs = elem.attributes.borrow_mut();
            let classes = attrs.get("class");
            if let Some(classes) = classes {
                let name = name.to_str()?;
                check_class_name(&name)?;
                if !split_classes(classes).any(|x| x == name.deref()) {
                    let new = format!("{classes} {name}");
                    attrs.insert("class", new);
                }
            } else {
                let name = name.to_string_lossy().to_string();
                check_class_name(&name)?;
                attrs.insert("class", name);
            }
            Ok(())
        })?,
    )?;
    table.raw_set(
        "remove_class",
        lua.create_function(|_, (node, name): (UserDataRef<LuaNodeRef>, LuaString)| {
            let elem = element(&node.0)?;
            let mut attrs = elem.attributes.borrow_mut();
            let classes = attrs.get("class");
            if let Some(classes) = classes {
                let name = name.to_str()?;
                check_class_name(&name)?;
                let mut new = String::new();
                for class in split_classes(classes).filter(|x| *x != name.deref()) {
                    if !new.is_empty() {
                        new.push(' ');
                    }
                    new.push_str(class);
                }
                attrs.insert("class", new);
            }
            Ok(())
        })?,
    )?;
    table.raw_set(
        "inner_text",
        lua.create_function(|lua, node: UserDataRef<LuaNodeRef>| {
            lua.create_string(inner_text(&node.0))
        })?,
    )?;
    table.raw_set(
        "strip_tags",
        lua.create_function(|lua, node: UserDataRef<LuaNodeRef>| {
            lua.create_string(strip_tags(&node.0))
        })?,
    )?;

    // Element tree manipulation
    // - Implemented in lua: HTML.append_root - legacy-only
    // - Implemented in lua: HTML.prepend_root - legacy-only
    // - Implemented in lua: HTML.replace - legacy-only
    // - Implemented in lua: HTML.replace_element
    // - Implemented in lua: HTML.replace_content
    // - Implemented in lua: HTML.delete
    // - Implemented in lua: HTML.wrap
    // - Implemented in lua: HTML.swap
    fn append_recurse(parent: &NodeRef, child: &NodeRef) {
        if child.as_document().is_some() {
            for node in child.children() {
                append_recurse(parent, &node);
            }
        } else {
            parent.append(child.clone());
        }
    }
    table.raw_set(
        "append_child",
        lua.create_function(
            |_, (parent, child): (UserDataRef<LuaNodeRef>, UserDataRef<LuaNodeRef>)| {
                append_recurse(&parent.0, &child.0);
                Ok(())
            },
        )?,
    )?;
    fn prepend_recurse(parent: &NodeRef, child: &NodeRef) {
        if child.as_document().is_some() {
            for node in child.children().rev() {
                prepend_recurse(parent, &node);
            }
        } else {
            parent.prepend(child.clone());
        }
    }
    table.raw_set(
        "prepend_child",
        lua.create_function(
            |_, (parent, child): (UserDataRef<LuaNodeRef>, UserDataRef<LuaNodeRef>)| {
                prepend_recurse(&parent.0, &child.0);
                Ok(())
            },
        )?,
    )?;
    fn insert_before_recurse(parent: &NodeRef, child: &NodeRef) {
        if child.as_document().is_some() {
            for node in child.children() {
                insert_before_recurse(parent, &node);
            }
        } else {
            parent.insert_before(child.clone());
        }
    }
    table.raw_set(
        "insert_before",
        lua.create_function(
            |_, (parent, child): (UserDataRef<LuaNodeRef>, UserDataRef<LuaNodeRef>)| {
                insert_before_recurse(&parent.0, &child.0);
                Ok(())
            },
        )?,
    )?;
    fn insert_after_recurse(parent: &NodeRef, child: &NodeRef) {
        if child.as_document().is_some() {
            for node in child.children().rev() {
                insert_after_recurse(parent, &node);
            }
        } else {
            parent.insert_after(child.clone());
        }
    }
    table.raw_set(
        "insert_after",
        lua.create_function(
            |_, (parent, child): (UserDataRef<LuaNodeRef>, UserDataRef<LuaNodeRef>)| {
                insert_after_recurse(&parent.0, &child.0);
                Ok(())
            },
        )?,
    )?;
    table.raw_set(
        "delete_element",
        lua.create_function(|_, elem: UserDataRef<LuaNodeRef>| {
            elem.0.detach();
            Ok(())
        })?,
    )?;
    table.raw_set(
        "delete_content",
        lua.create_function(|_, elem: UserDataRef<LuaNodeRef>| {
            for child in elem.0.children() {
                child.detach();
            }
            Ok(())
        })?,
    )?;
    table.raw_set(
        "unwrap",
        lua.create_function(|_, elem: UserDataRef<LuaNodeRef>| {
            for child in elem.0.children().rev() {
                insert_after_recurse(&elem.0, &child);
            }
            elem.0.detach();
            Ok(())
        })?,
    )?;

    // High-level convenience functions
    // - Implemented in lua: HTML.get_heading_level
    // - Implemented in lua: HTML.get_headings_tree

    // Node tests
    table.raw_set(
        "is_comment",
        lua.create_function(|_, elem: UserDataRef<LuaNodeRef>| {
            Ok(elem.0 .0.as_comment().is_some())
        })?,
    )?;
    table.raw_set(
        "is_doctype",
        lua.create_function(|_, elem: UserDataRef<LuaNodeRef>| {
            Ok(elem.0 .0.as_doctype().is_some())
        })?,
    )?;
    table.raw_set(
        "is_document",
        lua.create_function(|_, elem: UserDataRef<LuaNodeRef>| {
            Ok(elem.0 .0.as_document().is_some())
        })?,
    )?;
    table.raw_set(
        "is_element",
        lua.create_function(|_, elem: UserDataRef<LuaNodeRef>| {
            Ok(elem.0 .0.as_element().is_some())
        })?,
    )?;
    table.raw_set(
        "is_text",
        lua.create_function(|_, elem: UserDataRef<LuaNodeRef>| Ok(elem.0 .0.as_text().is_some()))?,
    )?;

    // Undocumented functions unknown to type checking.
    // Basically exclusively for internal use for debugging.
    table.raw_set(
        "_debug_node",
        lua.create_function(|_, elem: UserDataRef<LuaNodeRef>| {
            dbg!(&elem.0.data());
            Ok(())
        })?,
    )?;

    Ok(table)
}

#[derive(Clone, Debug)]
struct LuaNodeRef(NodeRef);
impl UserData for LuaNodeRef {
    fn add_fields<'lua, F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field("__type", "NodeRef");
    }
}
