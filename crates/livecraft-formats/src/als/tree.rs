//! Arena-backed lossless XML DOM tree for `.als` Live Set documents.

use std::collections::HashSet;

/// Default XML declaration emitted by Live 12.x `.als` files.
pub const DEFAULT_XML_DECLARATION: &str = r#"<?xml version="1.0" encoding="UTF-8"?>"#;

/// Arena index of a node inside an [`XmlTree`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct XmlNodeId(pub usize);

/// Quote delimiter surrounding an XML attribute value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum QuoteStyle {
    #[default]
    Double,
    Single,
}

/// Ordered XML attribute preserving its name, unescaped value, and quote style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlAttr {
    pub name: String,
    pub value: String,
    pub quote: QuoteStyle,
}

impl XmlAttr {
    /// Creates an attribute whose quote style defaults to single quotes when
    /// the value contains `"` and no `'`, and double quotes otherwise.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        let value = value.into();
        let quote = Self::preferred_quote_for(&value);
        Self {
            name: name.into(),
            value,
            quote,
        }
    }

    /// Creates an attribute with an explicit quote style.
    #[must_use]
    pub fn with_quote(
        name: impl Into<String>,
        value: impl Into<String>,
        quote: QuoteStyle,
    ) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            quote,
        }
    }

    /// Returns the preferred quote style for `value` using Live's convention.
    #[must_use]
    pub fn preferred_quote_for(value: &str) -> QuoteStyle {
        if value.contains('"') && !value.contains('\'') {
            QuoteStyle::Single
        } else {
            QuoteStyle::Double
        }
    }

    /// Returns the effective quote style that will be written on serialization.
    #[must_use]
    pub fn effective_quote(&self) -> QuoteStyle {
        if self.quote == QuoteStyle::Single
            || (self.value.contains('"') && !self.value.contains('\''))
        {
            QuoteStyle::Single
        } else {
            QuoteStyle::Double
        }
    }
}

/// Payload stored inside an [`XmlNode`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XmlNodeKind {
    Element {
        name: String,
        attrs: Vec<XmlAttr>,
        children: Vec<XmlNodeId>,
        /// `Some(true)` for `<Tag />`, `Some(false)` for `<Tag>...</Tag>`,
        /// and `None` to infer from whether `children` is empty.
        self_closing: Option<bool>,
    },
    Text(String),
    Comment(String),
    CData(String),
}

/// Single node inside an [`XmlTree`] arena.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlNode {
    pub id: XmlNodeId,
    pub parent: Option<XmlNodeId>,
    pub kind: XmlNodeKind,
}

impl XmlNode {
    /// Returns the tag name if this node is an element.
    #[must_use]
    pub fn element_name(&self) -> Option<&str> {
        match &self.kind {
            XmlNodeKind::Element { name, .. } => Some(name.as_str()),
            _ => None,
        }
    }

    /// Returns the attributes slice if this node is an element.
    #[must_use]
    pub fn attrs(&self) -> Option<&[XmlAttr]> {
        match &self.kind {
            XmlNodeKind::Element { attrs, .. } => Some(attrs.as_slice()),
            _ => None,
        }
    }

    /// Returns the child node IDs if this node is an element, or `&[]` otherwise.
    #[must_use]
    pub fn children(&self) -> &[XmlNodeId] {
        match &self.kind {
            XmlNodeKind::Element { children, .. } => children.as_slice(),
            _ => &[],
        }
    }

    /// Returns the inner string if this node is `Text` or `CData`.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match &self.kind {
            XmlNodeKind::Text(s) | XmlNodeKind::CData(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

/// Returns whether an element will be serialized in `<Tag />` self-closing form.
#[inline]
pub(crate) fn effective_self_closing(self_closing: Option<bool>, has_children: bool) -> bool {
    if has_children {
        false
    } else {
        self_closing.unwrap_or(true)
    }
}

/// Arena-allocated XML DOM tree preserving element order, attribute order,
/// attribute quote style, empty-tag formatting, comments, CDATA, and declaration.
#[derive(Debug, Clone)]
pub struct XmlTree {
    pub declaration: String,
    nodes: Vec<XmlNode>,
    root: XmlNodeId,
}

impl XmlTree {
    /// Creates a new tree with the default UTF-8 XML declaration and an empty root element.
    #[must_use]
    pub fn new(root_name: impl Into<String>) -> Self {
        Self::with_root(root_name, Vec::new())
    }

    /// Creates a new tree with the default UTF-8 XML declaration and a root element with `attrs`.
    #[must_use]
    pub fn with_root(name: impl Into<String>, attrs: Vec<XmlAttr>) -> Self {
        let root_id = XmlNodeId(0);
        let root = XmlNode {
            id: root_id,
            parent: None,
            kind: XmlNodeKind::Element {
                name: name.into(),
                attrs,
                children: Vec::new(),
                self_closing: None,
            },
        };
        Self {
            declaration: DEFAULT_XML_DECLARATION.to_owned(),
            nodes: vec![root],
            root: root_id,
        }
    }

    /// Replaces the XML declaration string (pass `""` to omit the declaration line).
    #[must_use]
    pub fn with_declaration(mut self, declaration: impl Into<String>) -> Self {
        self.declaration = declaration.into();
        self
    }

    /// Returns the root element's [`XmlNodeId`].
    #[must_use]
    pub fn root(&self) -> XmlNodeId {
        self.root
    }

    /// Returns the node for `id`, if it exists in this arena.
    #[must_use]
    pub fn node(&self, id: XmlNodeId) -> Option<&XmlNode> {
        self.nodes.get(id.0)
    }

    /// Returns a mutable reference to the node for `id`, if it exists in this arena.
    #[must_use]
    pub fn node_mut(&mut self, id: XmlNodeId) -> Option<&mut XmlNode> {
        self.nodes.get_mut(id.0)
    }

    /// Allocates a detached element node with no attributes and auto self-closing style.
    pub fn alloc_element(&mut self, name: impl Into<String>) -> XmlNodeId {
        self.alloc_element_with(name, Vec::new(), None)
    }

    /// Allocates a detached element node with explicit attributes and self-closing style.
    pub fn alloc_element_with(
        &mut self,
        name: impl Into<String>,
        attrs: Vec<XmlAttr>,
        self_closing: Option<bool>,
    ) -> XmlNodeId {
        self.alloc_kind(XmlNodeKind::Element {
            name: name.into(),
            attrs,
            children: Vec::new(),
            self_closing,
        })
    }

    /// Allocates a detached text node.
    pub fn alloc_text(&mut self, text: impl Into<String>) -> XmlNodeId {
        self.alloc_kind(XmlNodeKind::Text(text.into()))
    }

    /// Allocates a detached XML comment node.
    pub fn alloc_comment(&mut self, comment: impl Into<String>) -> XmlNodeId {
        self.alloc_kind(XmlNodeKind::Comment(comment.into()))
    }

    /// Allocates a detached CDATA section node.
    pub fn alloc_cdata(&mut self, cdata: impl Into<String>) -> XmlNodeId {
        self.alloc_kind(XmlNodeKind::CData(cdata.into()))
    }

    fn alloc_kind(&mut self, kind: XmlNodeKind) -> XmlNodeId {
        let id = XmlNodeId(self.nodes.len());
        self.nodes.push(XmlNode {
            id,
            parent: None,
            kind,
        });
        id
    }

    /// Returns the ordered child IDs of `id`, or `&[]` if `id` is missing or not an element.
    #[must_use]
    pub fn children(&self, id: XmlNodeId) -> &[XmlNodeId] {
        self.node(id).map(XmlNode::children).unwrap_or(&[])
    }

    /// Returns the first direct child element of `parent` whose tag name equals `name`.
    #[must_use]
    pub fn child(&self, parent: XmlNodeId, name: &str) -> Option<XmlNodeId> {
        self.children(parent).iter().copied().find(|&cid| {
            self.node(cid)
                .and_then(XmlNode::element_name)
                .is_some_and(|n| n == name)
        })
    }

    /// Returns a mutable reference to the first direct child element of `parent` named `name`.
    #[must_use]
    pub fn child_mut(&mut self, parent: XmlNodeId, name: &str) -> Option<&mut XmlNode> {
        let cid = self.child(parent, name)?;
        self.node_mut(cid)
    }

    /// Returns all direct child elements of `parent` whose tag name equals `name`, in order.
    #[must_use]
    pub fn children_named(&self, parent: XmlNodeId, name: &str) -> Vec<XmlNodeId> {
        self.children(parent)
            .iter()
            .copied()
            .filter(|&cid| {
                self.node(cid)
                    .and_then(XmlNode::element_name)
                    .is_some_and(|n| n == name)
            })
            .collect()
    }

    /// Searches descendants of `start` (excluding `start`) in depth-first pre-order for `name`.
    #[must_use]
    pub fn find_descendant_named(&self, start: XmlNodeId, name: &str) -> Option<XmlNodeId> {
        let mut stack: Vec<XmlNodeId> = self.children(start).iter().copied().rev().collect();
        while let Some(id) = stack.pop() {
            if let Some(node) = self.node(id)
                && let XmlNodeKind::Element {
                    name: ref el_name,
                    ref children,
                    ..
                } = node.kind
            {
                if el_name == name {
                    return Some(id);
                }
                stack.extend(children.iter().copied().rev());
            }
        }
        None
    }

    /// Searches the subtree at `start` (including `start`) in depth-first pre-order
    /// for the first element whose `Id` attribute equals `id_value`.
    #[must_use]
    pub fn find_by_id_attr(&self, start: XmlNodeId, id_value: &str) -> Option<XmlNodeId> {
        let mut stack = vec![start];
        while let Some(id) = stack.pop() {
            if self.attr(id, "Id") == Some(id_value) {
                return Some(id);
            }
            stack.extend(self.children(id).iter().copied().rev());
        }
        None
    }

    /// Returns the unescaped value of attribute `name` on element `id`.
    #[must_use]
    pub fn attr(&self, id: XmlNodeId, name: &str) -> Option<&str> {
        self.node(id)?
            .attrs()?
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.value.as_str())
    }

    /// Inserts or updates attribute `name` on element `id` in place.
    pub fn set_attr(
        &mut self,
        id: XmlNodeId,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> bool {
        let Some(XmlNode {
            kind: XmlNodeKind::Element { attrs, .. },
            ..
        }) = self.node_mut(id)
        else {
            return false;
        };
        let name = name.into();
        let value = value.into();
        if let Some(existing) = attrs.iter_mut().find(|a| a.name == name) {
            existing.quote = if value.contains('"') && !value.contains('\'') {
                QuoteStyle::Single
            } else if value.contains('\'') && !value.contains('"') {
                QuoteStyle::Double
            } else {
                existing.quote
            };
            existing.value = value;
        } else {
            attrs.push(XmlAttr::new(name, value));
        }
        true
    }

    /// Removes attribute `name` from element `id`, returning it if it existed.
    pub fn remove_attr(&mut self, id: XmlNodeId, name: &str) -> Option<XmlAttr> {
        let XmlNode {
            kind: XmlNodeKind::Element { attrs, .. },
            ..
        } = self.node_mut(id)?
        else {
            return None;
        };
        let pos = attrs.iter().position(|a| a.name == name)?;
        Some(attrs.remove(pos))
    }

    /// Returns the `Value` attribute of element `id`, if present.
    #[must_use]
    pub fn value_attr(&self, id: XmlNodeId) -> Option<&str> {
        self.attr(id, "Value")
    }

    /// Sets or updates the `Value` attribute on element `id`.
    pub fn set_value_attr(&mut self, id: XmlNodeId, value: impl Into<String>) -> bool {
        self.set_attr(id, "Value", value)
    }

    /// Returns the first direct child element named `name` under `parent`, or creates one.
    pub fn ensure_child(&mut self, parent: XmlNodeId, name: &str) -> Option<XmlNodeId> {
        if !matches!(
            self.node(parent).map(|n| &n.kind),
            Some(XmlNodeKind::Element { .. })
        ) {
            return None;
        }
        if let Some(existing) = self.child(parent, name) {
            return Some(existing);
        }
        let child = self.alloc_element(name);
        self.append_child(parent, child).then_some(child)
    }

    /// Appends `child` as the last child of `parent`, detaching it from any prior parent.
    pub fn append_child(&mut self, parent: XmlNodeId, child: XmlNodeId) -> bool {
        self.insert_child(parent, self.children(parent).len(), child)
    }

    /// Inserts `child` at `index` (clamped to `children.len()`) under `parent`.
    pub fn insert_child(&mut self, parent: XmlNodeId, index: usize, child: XmlNodeId) -> bool {
        if !self.can_attach(parent, child) {
            return false;
        }
        self.detach_from_parent(child);
        if let Some(XmlNode {
            kind: XmlNodeKind::Element { children, .. },
            ..
        }) = self.node_mut(parent)
        {
            let idx = index.min(children.len());
            children.insert(idx, child);
        }
        if let Some(child_node) = self.node_mut(child) {
            child_node.parent = Some(parent);
        }
        true
    }

    /// Removes `child` from `parent` if present, clearing `child`'s parent link.
    pub fn remove_child(&mut self, parent: XmlNodeId, child: XmlNodeId) -> bool {
        let Some(XmlNode {
            kind: XmlNodeKind::Element { children, .. },
            ..
        }) = self.node_mut(parent)
        else {
            return false;
        };
        let before = children.len();
        children.retain(|&c| c != child);
        if children.len() == before {
            return false;
        }
        if let Some(child_node) = self.node_mut(child) {
            child_node.parent = None;
        }
        true
    }

    /// Replaces all children of `parent` with `new_children`.
    pub fn replace_children(&mut self, parent: XmlNodeId, new_children: Vec<XmlNodeId>) -> bool {
        let mut seen = HashSet::with_capacity(new_children.len());
        if !new_children
            .iter()
            .all(|&cid| seen.insert(cid) && self.can_attach(parent, cid))
        {
            return false;
        }
        self.clear_children(parent);
        for &cid in &new_children {
            self.detach_from_parent(cid);
            if let Some(child_node) = self.node_mut(cid) {
                child_node.parent = Some(parent);
            }
        }
        if let Some(XmlNode {
            kind: XmlNodeKind::Element { children, .. },
            ..
        }) = self.node_mut(parent)
        {
            *children = new_children;
            return true;
        }
        false
    }

    /// Detaches and clears all children of `parent`.
    pub fn clear_children(&mut self, parent: XmlNodeId) -> bool {
        let Some(XmlNode {
            kind: XmlNodeKind::Element { children, .. },
            ..
        }) = self.node_mut(parent)
        else {
            return false;
        };
        let old_children = std::mem::take(children);
        for cid in old_children {
            if let Some(child_node) = self.node_mut(cid) {
                child_node.parent = None;
            }
        }
        true
    }

    /// Returns the concatenated text (and CDATA) inside the subtree rooted at `id`.
    #[must_use]
    pub fn text_content(&self, id: XmlNodeId) -> String {
        let mut out = String::new();
        let mut stack = vec![id];
        while let Some(curr) = stack.pop() {
            if let Some(node) = self.node(curr) {
                match &node.kind {
                    XmlNodeKind::Text(s) | XmlNodeKind::CData(s) => out.push_str(s),
                    XmlNodeKind::Element { children, .. } => {
                        stack.extend(children.iter().copied().rev());
                    }
                    XmlNodeKind::Comment(_) => {}
                }
            }
        }
        out
    }

    /// Sets the text content of `id` (updating a `Text` node or replacing an element's children).
    pub fn set_text_content(&mut self, id: XmlNodeId, text: impl Into<String>) -> bool {
        let text = text.into();
        match self.node_mut(id) {
            Some(XmlNode {
                kind: XmlNodeKind::Text(existing),
                ..
            }) => {
                *existing = text;
                true
            }
            Some(XmlNode {
                kind: XmlNodeKind::Element { .. },
                ..
            }) => {
                self.clear_children(id);
                if !text.is_empty() {
                    let tid = self.alloc_text(text);
                    self.append_child(id, tid);
                }
                true
            }
            _ => false,
        }
    }

    /// Deep-clones the subtree rooted at `source_root` from `source` into this arena.
    pub fn import_subtree(
        &mut self,
        source: &XmlTree,
        source_root: XmlNodeId,
    ) -> Option<XmlNodeId> {
        let root_new = self.alloc_clone_of(source.node(source_root)?);
        let mut stack = vec![(source_root, root_new)];
        while let Some((s_parent, t_parent)) = stack.pop() {
            for &s_child in source.children(s_parent) {
                let child_node = source.node(s_child)?;
                let t_child = self.alloc_clone_of(child_node);
                self.append_child(t_parent, t_child);
                if matches!(child_node.kind, XmlNodeKind::Element { .. }) {
                    stack.push((s_child, t_child));
                }
            }
        }
        Some(root_new)
    }

    fn alloc_clone_of(&mut self, node: &XmlNode) -> XmlNodeId {
        match &node.kind {
            XmlNodeKind::Element {
                name,
                attrs,
                self_closing,
                ..
            } => self.alloc_element_with(name.clone(), attrs.clone(), *self_closing),
            XmlNodeKind::Text(s) => self.alloc_text(s.clone()),
            XmlNodeKind::Comment(s) => self.alloc_comment(s.clone()),
            XmlNodeKind::CData(s) => self.alloc_cdata(s.clone()),
        }
    }

    fn is_ancestor(&self, ancestor: XmlNodeId, mut node: XmlNodeId) -> bool {
        for _ in 0..=self.nodes.len() {
            if node == ancestor {
                return true;
            }
            let Some(parent) = self.node(node).and_then(|n| n.parent) else {
                return false;
            };
            node = parent;
        }
        false
    }

    fn can_attach(&self, parent: XmlNodeId, child: XmlNodeId) -> bool {
        matches!(
            self.node(parent).map(|n| &n.kind),
            Some(XmlNodeKind::Element { .. })
        ) && self.node(child).is_some()
            && child != self.root
            && !self.is_ancestor(child, parent)
    }

    fn detach_from_parent(&mut self, child: XmlNodeId) {
        if let Some(pid) = self.node(child).and_then(|n| n.parent) {
            if let Some(XmlNode {
                kind: XmlNodeKind::Element { children, .. },
                ..
            }) = self.node_mut(pid)
            {
                children.retain(|&c| c != child);
            }
            if let Some(child_node) = self.node_mut(child) {
                child_node.parent = None;
            }
        }
    }
}

impl PartialEq for XmlTree {
    fn eq(&self, other: &Self) -> bool {
        if self.declaration != other.declaration {
            return false;
        }
        let mut stack = vec![(self.root, other.root)];
        while let Some((a_id, b_id)) = stack.pop() {
            let (Some(a), Some(b)) = (self.node(a_id), other.node(b_id)) else {
                return false;
            };
            match (&a.kind, &b.kind) {
                (
                    XmlNodeKind::Element {
                        name: n1,
                        attrs: a1,
                        children: c1,
                        self_closing: s1,
                    },
                    XmlNodeKind::Element {
                        name: n2,
                        attrs: a2,
                        children: c2,
                        self_closing: s2,
                    },
                ) => {
                    let eq_attrs = a1.len() == a2.len()
                        && a1.iter().zip(a2).all(|(x, y)| {
                            x.name == y.name
                                && x.value == y.value
                                && x.effective_quote() == y.effective_quote()
                        });
                    let eq_sc = effective_self_closing(*s1, !c1.is_empty())
                        == effective_self_closing(*s2, !c2.is_empty());
                    if n1 != n2 || !eq_attrs || !eq_sc || c1.len() != c2.len() {
                        return false;
                    }
                    stack.extend(c1.iter().copied().zip(c2.iter().copied()));
                }
                (XmlNodeKind::Text(x), XmlNodeKind::Text(y))
                | (XmlNodeKind::Comment(x), XmlNodeKind::Comment(y))
                | (XmlNodeKind::CData(x), XmlNodeKind::CData(y))
                    if x == y => {}
                _ => return false,
            }
        }
        true
    }
}

impl Eq for XmlTree {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::als::read::read_xml_tree;
    use crate::als::write::{XmlFormatMode, write_xml_tree};
    use proptest::prelude::*;
    use std::collections::BTreeSet;

    #[derive(Debug, Clone)]
    enum GenNode {
        Element(String, Vec<XmlAttr>, Vec<GenNode>, Option<bool>),
        Text(String),
        Comment(String),
        CData(String),
    }

    fn arb_attrs() -> impl Strategy<Value = Vec<XmlAttr>> {
        let name = "[A-Za-z][A-Za-z0-9_-]{0,10}";
        let val = proptest::string::string_regex("[^\0]{0,30}").expect("valid attr regex");
        proptest::collection::vec((name, val, any::<bool>()), 0..=4).prop_map(|raw| {
            let mut seen = BTreeSet::new();
            raw.into_iter()
                .filter(|(k, _, _)| seen.insert(k.clone()))
                .map(|(n, v, single)| {
                    let q = if (v.contains('"') && !v.contains('\'')) || single {
                        QuoteStyle::Single
                    } else {
                        QuoteStyle::Double
                    };
                    XmlAttr::with_quote(n, v, q)
                })
                .collect()
        })
    }

    fn arb_element(depth: u32) -> impl Strategy<Value = GenNode> {
        let name = "[A-Za-z][A-Za-z0-9_-]{0,10}";
        let leaf = (name, arb_attrs(), any::<bool>())
            .prop_map(|(n, a, sc)| GenNode::Element(n, a, Vec::new(), Some(sc)));
        if depth == 0 {
            return leaf.boxed();
        }
        let text = proptest::string::string_regex("[^\0]*[^\0 \t\r\n][^\0]*").expect("text regex");
        let comment = proptest::string::string_regex("[^\0-]{0,20}").expect("comment regex");
        let cdata = proptest::string::string_regex("[^\0\\]>]{0,25}").expect("cdata regex");
        let child = prop_oneof![
            3 => arb_element(depth - 1),
            1 => text.prop_map(GenNode::Text),
            1 => comment.prop_map(GenNode::Comment),
            1 => cdata.prop_map(GenNode::CData),
        ];
        (
            name,
            arb_attrs(),
            proptest::collection::vec(child, 0..=4),
            any::<bool>(),
        )
            .prop_map(|(n, a, raw, empty_sc)| {
                let mut kids = Vec::with_capacity(raw.len());
                for c in raw {
                    if !matches!(c, GenNode::Text(_))
                        || !matches!(kids.last(), Some(GenNode::Text(_)))
                    {
                        kids.push(c);
                    }
                }
                let sc = Some(if kids.is_empty() { empty_sc } else { false });
                GenNode::Element(n, a, kids, sc)
            })
            .boxed()
    }

    fn build_tree(decl: String, root: GenNode) -> XmlTree {
        let GenNode::Element(name, attrs, kids, sc) = root else {
            unreachable!("root is always element");
        };
        let mut tree = XmlTree::with_root(name, attrs).with_declaration(decl);
        let root_id = tree.root();
        if let Some(XmlNode {
            kind: XmlNodeKind::Element { self_closing, .. },
            ..
        }) = tree.node_mut(root_id)
        {
            *self_closing = sc;
        }
        let mut stack = vec![(root_id, kids)];
        while let Some((pid, list)) = stack.pop() {
            for kid in list {
                let cid = match kid {
                    GenNode::Element(n, a, c, sc) => {
                        let id = tree.alloc_element_with(n, a, sc);
                        if !c.is_empty() {
                            stack.push((id, c));
                        }
                        id
                    }
                    GenNode::Text(s) => tree.alloc_text(s),
                    GenNode::Comment(s) => tree.alloc_comment(s),
                    GenNode::CData(s) => tree.alloc_cdata(s),
                };
                tree.append_child(pid, cid);
            }
        }
        tree
    }

    fn arb_xml_tree() -> impl Strategy<Value = XmlTree> {
        let decl = prop_oneof![
            Just(String::new()),
            Just(DEFAULT_XML_DECLARATION.to_owned()),
            Just(r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>"#.to_owned()),
        ];
        (decl, arb_element(3)).prop_map(|(d, r)| build_tree(d, r))
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1024))]
        #[test]
        fn random_xml_tree_roundtrips_losslessly(tree in arb_xml_tree()) {
            let xml = write_xml_tree(&tree, XmlFormatMode::LiveExact).expect("serialize XmlTree");
            let parsed = read_xml_tree(&xml).expect("re-parse XmlTree");
            prop_assert_eq!(parsed, tree);
        }
    }
}
