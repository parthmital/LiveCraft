//! Integration tests for [`XmlTree`] arena manipulation and template round-tripping.

use livecraft_formats::als::tree::{QuoteStyle, XmlAttr, XmlNode, XmlTree};

#[test]
fn arena_crud_and_search_methods() {
    let mut tree = XmlTree::with_root("Ableton", vec![XmlAttr::new("MajorVersion", "5")]);
    let root = tree.root();
    let live_set = tree.ensure_child(root, "LiveSet").expect("LiveSet created");
    assert_eq!(tree.ensure_child(root, "LiveSet"), Some(live_set));
    assert!(tree.child_mut(root, "LiveSet").is_some());

    let tracks = tree.alloc_element("Tracks");
    assert!(tree.append_child(live_set, tracks));

    let m1 = tree.alloc_element_with("MidiTrack", vec![XmlAttr::new("Id", "10")], Some(false));
    let m2 = tree.alloc_element_with("MidiTrack", vec![XmlAttr::new("Id", "11")], None);
    assert!(tree.append_child(tracks, m2));
    assert!(tree.insert_child(tracks, 0, m1));
    assert_eq!(tree.children_named(tracks, "MidiTrack"), vec![m1, m2]);

    let view_data = tree.alloc_element("ViewData");
    assert!(tree.set_attr(view_data, "OrderA", "1"));
    assert!(tree.set_value_attr(view_data, r#"{"mode": "play"}"#));
    assert!(tree.set_attr(view_data, "OrderA", "2"));
    assert_eq!(tree.attr(view_data, "OrderA"), Some("2"));
    assert_eq!(tree.value_attr(view_data), Some(r#"{"mode": "play"}"#));
    let attrs = tree
        .node(view_data)
        .and_then(XmlNode::attrs)
        .expect("attrs");
    assert_eq!(attrs[0].name, "OrderA");
    assert_eq!(attrs[1].quote, QuoteStyle::Single);
    assert!(tree.append_child(m1, view_data));

    assert_eq!(
        tree.find_descendant_named(root, "ViewData"),
        Some(view_data)
    );
    assert_eq!(tree.find_by_id_attr(root, "11"), Some(m2));
    assert!(tree.set_text_content(m2, "hello"));
    assert_eq!(tree.text_content(m2), "hello");
    assert!(tree.set_text_content(m2, ""));
    assert_eq!(tree.text_content(m2), "");

    assert!(!tree.append_child(m1, root));
    assert!(!tree.append_child(view_data, m1));
    assert!(tree.remove_child(tracks, m2));
    assert_eq!(tree.children_named(tracks, "MidiTrack"), vec![m1]);
    assert!(tree.remove_attr(view_data, "Value").is_some());
    assert_eq!(tree.value_attr(view_data), None);

    let replacement = tree.alloc_element("AudioTrack");
    assert!(tree.replace_children(tracks, vec![replacement]));
    assert_eq!(tree.children(tracks), &[replacement]);
    assert!(tree.clear_children(tracks));
    assert!(tree.children(tracks).is_empty());

    let mut copy_tree = XmlTree::new("Wrapper");
    let imported = copy_tree.import_subtree(&tree, live_set).expect("import");
    assert!(copy_tree.append_child(copy_tree.root(), imported));
    assert!(
        copy_tree
            .find_descendant_named(copy_tree.root(), "Tracks")
            .is_some()
    );
}
