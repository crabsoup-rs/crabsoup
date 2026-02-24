use html5ever::interface::{ElemName, ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::{Attribute, LocalName, Namespace, ParseOpts, QualName};
use std::borrow::Cow;
use std::cell::{Cell, RefCell};

struct IsDocumentTreeSink {
    handle_id: Cell<usize>,
    is_document: Cell<bool>,
    elements: RefCell<Vec<Option<(Namespace, LocalName)>>>,
}
impl Default for IsDocumentTreeSink {
    fn default() -> Self {
        IsDocumentTreeSink {
            handle_id: 0.into(),
            is_document: false.into(),
            elements: RefCell::new(vec![]),
        }
    }
}

impl IsDocumentTreeSink {
    fn handle(&self) -> usize {
        let id = self.handle_id.get();
        self.handle_id.set(id + 1);
        id
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct ExpandedName {
    pub ns: Namespace,
    pub local: LocalName,
}
impl ElemName for ExpandedName {
    fn ns(&self) -> &Namespace {
        &self.ns
    }
    fn local_name(&self) -> &LocalName {
        &self.local
    }
}

impl TreeSink for IsDocumentTreeSink {
    type Handle = usize;
    type Output = bool;
    type ElemName<'a> = ExpandedName;

    fn finish(self) -> Self::Output {
        self.is_document.get()
    }

    fn parse_error(&self, _: Cow<'static, str>) {}

    fn get_document(&self) -> Self::Handle {
        usize::MAX
    }

    fn elem_name<'a>(&'a self, h: &'a Self::Handle) -> ExpandedName {
        let t = &self.elements.borrow()[*h];
        let t = t.as_ref().unwrap();
        ExpandedName { ns: t.0.clone(), local: t.1.clone() }
    }

    fn create_element(&self, name: QualName, _: Vec<Attribute>, _: ElementFlags) -> Self::Handle {
        self.elements.borrow_mut().push(Some((name.ns, name.local)));
        self.handle()
    }

    fn create_comment(&self, _: StrTendril) -> Self::Handle {
        self.elements.borrow_mut().push(None);
        self.handle()
    }

    fn create_pi(&self, _: StrTendril, _: StrTendril) -> Self::Handle {
        self.elements.borrow_mut().push(None);
        self.handle()
    }

    fn append(&self, _: &Self::Handle, _: NodeOrText<Self::Handle>) {}

    fn append_based_on_parent_node(
        &self,
        _: &Self::Handle,
        _: &Self::Handle,
        _: NodeOrText<Self::Handle>,
    ) {
    }

    fn append_doctype_to_document(&self, _: StrTendril, _: StrTendril, _: StrTendril) {
        self.is_document.set(true);
    }

    fn get_template_contents(&self, _: &Self::Handle) -> Self::Handle {
        todo!()
    }

    fn same_node(&self, x: &Self::Handle, y: &Self::Handle) -> bool {
        x == y
    }

    fn set_quirks_mode(&self, _: QuirksMode) {}

    fn append_before_sibling(&self, _: &Self::Handle, _: NodeOrText<Self::Handle>) {}

    fn add_attrs_if_missing(&self, _: &Self::Handle, _: Vec<Attribute>) {}

    fn remove_from_parent(&self, _: &Self::Handle) {}

    fn reparent_children(&self, _: &Self::Handle, _: &Self::Handle) {}
}

pub fn is_document(source: &str) -> bool {
    html5ever::parse_document(IsDocumentTreeSink::default(), ParseOpts::default()).one(source)
}
