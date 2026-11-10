//! Lossless streaming XML and gzip `.als` reader into [`XmlTree`].

use std::fs::File;
use std::io::{BufReader, Cursor, ErrorKind, Read};
use std::path::Path;

use flate2::read::GzDecoder;
use quick_xml::XmlVersion;
use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

use super::AlsError;
use super::tree::{QuoteStyle, XmlAttr, XmlNodeId, XmlNodeKind, XmlTree};

/// Largest decompressed `.als` XML accepted, in bytes (512 MiB, LC-001).
pub const MAX_XML_BYTES: u64 = 512 * 1024 * 1024;

/// Maximum nested XML element depth permitted to guard against stack or OOM abuse.
const MAX_XML_DEPTH: usize = 4096;

const GZIP_MAGIC: [u8; 2] = [0x1F, 0x8B];

/// Reads a gzip-compressed `.als` file or plain UTF-8 `.xml` file into an [`XmlTree`].
pub fn read_als_tree<P: AsRef<Path>>(path: P) -> Result<XmlTree, AlsError> {
    let xml = decompress_als_or_xml(path)?;
    read_xml_tree(&xml)
}

/// Reads either a gzip `.als` file or a plain UTF-8 `.xml` file up to [`MAX_XML_BYTES`].
pub fn decompress_als_or_xml<P: AsRef<Path>>(path: P) -> Result<String, AlsError> {
    let file = File::open(path)?;
    decompress_reader(BufReader::new(file), MAX_XML_BYTES)
}

/// Decompresses gzip data (when `[0x1F, 0x8B]` magic is present) or reads plain UTF-8
/// from `reader`, rejecting output larger than `limit` bytes with [`AlsError::TooLarge`].
pub fn decompress_reader<R: Read>(mut reader: R, limit: u64) -> Result<String, AlsError> {
    let mut prefix = [0u8; 2];
    let mut prefix_len = 0usize;
    while prefix_len < prefix.len() {
        match reader.read(&mut prefix[prefix_len..]) {
            Ok(0) => break,
            Ok(n) => prefix_len += n,
            Err(err) if err.kind() == ErrorKind::Interrupted => continue,
            Err(err) => return Err(AlsError::Io(err)),
        }
    }

    let is_gzip = prefix_len == 2 && prefix == GZIP_MAGIC;
    let mut bytes = Vec::new();
    if is_gzip {
        let chained = Cursor::new(&prefix[..prefix_len]).chain(reader);
        GzDecoder::new(chained)
            .take(limit.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|e| AlsError::Decompress(format!("Failed to decompress gzip stream: {e}")))?;
        if bytes.len() as u64 > limit {
            return Err(AlsError::TooLarge { limit });
        }
    } else {
        if prefix_len as u64 > limit {
            return Err(AlsError::TooLarge { limit });
        }
        bytes.extend_from_slice(&prefix[..prefix_len]);
        let remaining_cap = limit.saturating_sub(prefix_len as u64).saturating_add(1);
        reader.take(remaining_cap).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err(AlsError::TooLarge { limit });
        }
    }

    String::from_utf8(bytes)
        .map_err(|e| AlsError::Decompress(format!("Live Set XML is not UTF-8: {e}")))
}

/// Parses an XML string into a lossless [`XmlTree`], preserving XML declaration,
/// attribute order, attribute quote styles, self-closing vs empty-pair tags,
/// meaningful text nodes, comments, and CDATA sections.
pub fn read_xml_tree(xml: &str) -> Result<XmlTree, AlsError> {
    let xml = xml.strip_prefix('\u{FEFF}').unwrap_or(xml);
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut declaration: Option<String> = None;
    let mut tree: Option<XmlTree> = None;
    let mut stack: Vec<XmlNodeId> = Vec::new();
    let mut pending_text = String::new();
    let mut pending_had_ref = false;

    loop {
        match reader.read_event()? {
            Event::Decl(decl) => {
                if tree.is_some() || declaration.is_some() {
                    return Err(AlsError::InvalidFormat(
                        "unexpected or duplicate XML declaration".to_owned(),
                    ));
                }
                decl.xml_version()?;
                declaration = Some(format!("<?{}?>", decl.as_ref()));
            }
            Event::Start(start) => {
                flush_pending_text(&mut tree, &stack, &mut pending_text, &mut pending_had_ref)?;
                let elem_id = push_element(&mut tree, &mut declaration, &stack, &start, false)?;
                if stack.len() >= MAX_XML_DEPTH {
                    return Err(AlsError::InvalidFormat(format!(
                        "XML nesting depth exceeds {MAX_XML_DEPTH}"
                    )));
                }
                stack.push(elem_id);
            }
            Event::Empty(empty) => {
                flush_pending_text(&mut tree, &stack, &mut pending_text, &mut pending_had_ref)?;
                push_element(&mut tree, &mut declaration, &stack, &empty, true)?;
            }
            Event::End(end) => {
                flush_pending_text(&mut tree, &stack, &mut pending_text, &mut pending_had_ref)?;
                let Some(closed_id) = stack.pop() else {
                    return Err(AlsError::InvalidFormat(format!(
                        "unexpected closing tag `</{}>`",
                        end.name().0
                    )));
                };
                let expected = tree
                    .as_ref()
                    .and_then(|t| t.node(closed_id))
                    .and_then(|n| n.element_name());
                if expected != Some(end.name().0) {
                    return Err(AlsError::InvalidFormat(format!(
                        "mismatched closing tag `</{}>`",
                        end.name().0
                    )));
                }
            }
            Event::Text(text) => {
                let normalized = text.xml10_content();
                pending_text.push_str(&normalized);
            }
            Event::GeneralRef(gref) => {
                if gref.is_char_ref() {
                    let ch = gref.resolve_char_ref()?.ok_or_else(|| {
                        AlsError::InvalidFormat("empty XML character reference".to_owned())
                    })?;
                    pending_text.push(ch);
                    pending_had_ref = true;
                } else if let Some(resolved) = resolve_predefined_entity(gref.as_ref()) {
                    pending_text.push_str(resolved);
                    pending_had_ref = true;
                } else {
                    return Err(AlsError::InvalidFormat(format!(
                        "unknown XML entity `&{};`",
                        gref.as_ref()
                    )));
                }
            }
            Event::Comment(comment) => {
                flush_pending_text(&mut tree, &stack, &mut pending_text, &mut pending_had_ref)?;
                if let (Some(t), Some(&parent_id)) = (tree.as_mut(), stack.last()) {
                    let cid = t.alloc_comment(comment.as_ref().to_owned());
                    t.append_child(parent_id, cid);
                }
            }
            Event::CData(cdata) => {
                flush_pending_text(&mut tree, &stack, &mut pending_text, &mut pending_had_ref)?;
                let (Some(t), Some(&parent_id)) = (tree.as_mut(), stack.last()) else {
                    return Err(AlsError::InvalidFormat(
                        "CDATA section outside root element".to_owned(),
                    ));
                };
                let cid = t.alloc_cdata(cdata.as_ref().to_owned());
                t.append_child(parent_id, cid);
            }
            Event::PI(_) | Event::DocType(_) => {
                flush_pending_text(&mut tree, &stack, &mut pending_text, &mut pending_had_ref)?;
            }
            Event::Eof => break,
        }
    }

    flush_pending_text(&mut tree, &stack, &mut pending_text, &mut pending_had_ref)?;
    if !stack.is_empty() {
        return Err(AlsError::InvalidFormat(
            "unexpected EOF with unclosed XML elements".to_owned(),
        ));
    }
    tree.ok_or_else(|| {
        AlsError::InvalidFormat("empty XML document: missing root element".to_owned())
    })
}

fn push_element(
    tree: &mut Option<XmlTree>,
    declaration: &mut Option<String>,
    stack: &[XmlNodeId],
    start: &BytesStart<'_>,
    is_empty: bool,
) -> Result<XmlNodeId, AlsError> {
    let name = start.name().0;
    if name.is_empty() {
        return Err(AlsError::InvalidFormat("empty XML element name".to_owned()));
    }
    let attrs = parse_attributes(start)?;
    let self_closing = Some(is_empty);

    if let Some(&parent_id) = stack.last() {
        let Some(t) = tree.as_mut() else {
            return Err(AlsError::InvalidFormat(
                "missing root XML element".to_owned(),
            ));
        };
        let id = t.alloc_element_with(name, attrs, self_closing);
        t.append_child(parent_id, id);
        Ok(id)
    } else {
        if tree.is_some() {
            return Err(AlsError::InvalidFormat(
                "multiple root XML elements".to_owned(),
            ));
        }
        let decl = declaration.take().unwrap_or_default();
        let mut t = XmlTree::with_root(name, attrs).with_declaration(decl);
        let root_id = t.root();
        if let Some(root_node) = t.node_mut(root_id)
            && let XmlNodeKind::Element {
                self_closing: sc, ..
            } = &mut root_node.kind
        {
            *sc = self_closing;
        }
        *tree = Some(t);
        Ok(root_id)
    }
}

fn flush_pending_text(
    tree: &mut Option<XmlTree>,
    stack: &[XmlNodeId],
    pending_text: &mut String,
    pending_had_ref: &mut bool,
) -> Result<(), AlsError> {
    if pending_text.is_empty() {
        *pending_had_ref = false;
        return Ok(());
    }
    let meaningful = *pending_had_ref || !is_xml_whitespace(pending_text);
    let text = std::mem::take(pending_text);
    *pending_had_ref = false;

    if !meaningful {
        return Ok(());
    }
    let (Some(t), Some(&parent_id)) = (tree.as_mut(), stack.last()) else {
        return Err(AlsError::InvalidFormat(
            "unexpected text outside root XML element".to_owned(),
        ));
    };
    let tid = t.alloc_text(text);
    t.append_child(parent_id, tid);
    Ok(())
}

fn is_xml_whitespace(s: &str) -> bool {
    s.bytes().all(is_xml_ws_byte)
}

#[inline]
fn is_xml_ws_byte(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n')
}

fn parse_attributes(start: &BytesStart<'_>) -> Result<Vec<XmlAttr>, AlsError> {
    let raw = start.attributes_raw().as_bytes();
    let mut cursor = 0usize;
    let mut attrs = Vec::new();

    for attr_res in start.attributes() {
        let attr = attr_res.map_err(|err| AlsError::Xml(err.into()))?;
        let quote = scan_next_attr_quote(raw, &mut cursor)?;
        let value = attr
            .normalized_value(XmlVersion::Explicit1_0)
            .map_err(AlsError::Xml)?
            .into_owned();
        attrs.push(XmlAttr {
            name: attr.key.0.to_owned(),
            value,
            quote,
        });
    }

    Ok(attrs)
}

fn scan_next_attr_quote(raw: &[u8], cursor: &mut usize) -> Result<QuoteStyle, AlsError> {
    let mut i = *cursor;
    while i < raw.len() && is_xml_ws_byte(raw[i]) {
        i += 1;
    }
    while i < raw.len() && raw[i] != b'=' && !is_xml_ws_byte(raw[i]) {
        i += 1;
    }
    while i < raw.len() && is_xml_ws_byte(raw[i]) {
        i += 1;
    }
    if raw.get(i) != Some(&b'=') {
        return Err(AlsError::InvalidFormat(
            "malformed XML attribute: expected '='".to_owned(),
        ));
    }
    i += 1;
    while i < raw.len() && is_xml_ws_byte(raw[i]) {
        i += 1;
    }
    let (quote_byte, style) = match raw.get(i) {
        Some(b'"') => (b'"', QuoteStyle::Double),
        Some(b'\'') => (b'\'', QuoteStyle::Single),
        _ => {
            return Err(AlsError::InvalidFormat(
                "malformed XML attribute: expected quote".to_owned(),
            ));
        }
    };
    i += 1;
    while i < raw.len() && raw[i] != quote_byte {
        i += 1;
    }
    if raw.get(i) != Some(&quote_byte) {
        return Err(AlsError::InvalidFormat(
            "malformed XML attribute: unclosed quote".to_owned(),
        ));
    }
    *cursor = i + 1;
    Ok(style)
}

#[cfg(test)]
mod tests {
    use super::super::tree::XmlNode;
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use proptest::prelude::*;
    use std::io::Write;

    #[test]
    fn decompress_reader_accepts_plain_xml_and_gzip_and_enforces_limit() {
        let xml = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Ableton />\n";
        let plain = decompress_reader(xml.as_bytes(), 1024).expect("plain XML");
        assert_eq!(plain, xml);

        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(xml.as_bytes()).expect("compress");
        let gz = enc.finish().expect("finish gzip");
        let decoded = decompress_reader(gz.as_slice(), 1024).expect("gzip XML");
        assert_eq!(decoded, xml);

        let err_plain = decompress_reader(xml.as_bytes(), 10).unwrap_err();
        assert!(matches!(err_plain, AlsError::TooLarge { limit: 10 }));

        let err_gz = decompress_reader(gz.as_slice(), 10).unwrap_err();
        assert!(matches!(err_gz, AlsError::TooLarge { limit: 10 }));
    }

    #[test]
    fn read_xml_tree_preserves_quotes_self_closing_comments_cdata_and_text() {
        let src = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<Ableton MajorVersion=\"5\" ViewData='{\"k\": [1, 2]}'>\n",
            "\t<!-- hello -->\n",
            "\t<Envelopes></Envelopes>\n",
            "\t<Tag />\n",
            "\t<Data>a &amp; b &lt; c</Data>\n",
            "\t<Raw><![CDATA[<tag>&raw]]></Raw>\n",
            "</Ableton>\n",
        );
        let tree = read_xml_tree(src).expect("parse fixture");
        let root = tree.root();
        let attrs = tree.node(root).and_then(XmlNode::attrs).expect("attrs");
        assert_eq!(attrs[0].quote, QuoteStyle::Double);
        assert_eq!(attrs[1].quote, QuoteStyle::Single);
        assert_eq!(attrs[1].value, r#"{"k": [1, 2]}"#);

        let children = tree.children(root);
        assert_eq!(children.len(), 5);
        assert!(matches!(
            tree.node(children[0]).map(|n| &n.kind),
            Some(XmlNodeKind::Comment(s)) if s == " hello "
        ));
        assert!(matches!(
            tree.node(children[1]).map(|n| &n.kind),
            Some(XmlNodeKind::Element {
                self_closing: Some(false),
                ..
            })
        ));
        assert!(matches!(
            tree.node(children[2]).map(|n| &n.kind),
            Some(XmlNodeKind::Element {
                self_closing: Some(true),
                ..
            })
        ));
        assert_eq!(tree.text_content(children[3]), "a & b < c");
        assert!(matches!(
            tree.node(tree.children(children[4])[0]).map(|n| &n.kind),
            Some(XmlNodeKind::CData(s)) if s == "<tag>&raw"
        ));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1024))]
        #[test]
        fn arbitrary_bytes_never_panic_in_decompress_or_xml_reader(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
            let _ = decompress_reader(bytes.as_slice(), 4096);
            if let Ok(text) = std::str::from_utf8(&bytes) {
                let _ = read_xml_tree(text);
            }
        }
    }
}
