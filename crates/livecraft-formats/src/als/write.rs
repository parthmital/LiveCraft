//! Lossless `.als` XML and gzip writer matching Live 12.x formatting conventions.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use flate2::Compression;
use flate2::write::GzEncoder;

use super::AlsError;
use super::tree::{QuoteStyle, XmlAttr, XmlNodeId, XmlNodeKind, XmlTree, effective_self_closing};

/// Output formatting mode for [`write_xml_tree`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XmlFormatMode {
    /// Preserves exact attribute order, quote style, and empty-tag style.
    LiveExact,
    /// Sorts attributes stably by name for VCS-friendly diffs.
    PrettyCanonical,
}

enum WriteOp<'a> {
    Node {
        id: XmlNodeId,
        depth: usize,
        parent_inline: bool,
    },
    EndElement {
        name: &'a str,
        depth: usize,
        indent_before_close: bool,
        newline_after_close: bool,
    },
}

/// Serializes an [`XmlTree`] to UTF-8 XML using LF line endings and single-tab indentation.
pub fn write_xml_tree(tree: &XmlTree, mode: XmlFormatMode) -> Result<String, AlsError> {
    let mut out = String::with_capacity(4096);
    if !tree.declaration.is_empty() {
        out.push_str(&tree.declaration);
        out.push('\n');
    }

    let mut stack = vec![WriteOp::Node {
        id: tree.root(),
        depth: 0,
        parent_inline: false,
    }];

    while let Some(op) = stack.pop() {
        match op {
            WriteOp::EndElement {
                name,
                depth,
                indent_before_close,
                newline_after_close,
            } => {
                if indent_before_close {
                    push_tabs(&mut out, depth);
                }
                out.push_str("</");
                out.push_str(name);
                out.push('>');
                if newline_after_close {
                    out.push('\n');
                }
            }
            WriteOp::Node {
                id,
                depth,
                parent_inline,
            } => {
                let node = tree.node(id).ok_or_else(|| {
                    AlsError::InvalidFormat(format!("dangling XmlNodeId({})", id.0))
                })?;
                match &node.kind {
                    XmlNodeKind::Text(text) => {
                        escape_text(&mut out, text);
                    }
                    XmlNodeKind::Comment(comment) => {
                        if comment.contains("--") || comment.ends_with('-') {
                            return Err(AlsError::InvalidFormat(
                                "XML comment cannot contain `--` or end with `-`".to_owned(),
                            ));
                        }
                        if !parent_inline {
                            push_tabs(&mut out, depth);
                        }
                        out.push_str("<!--");
                        out.push_str(comment);
                        out.push_str("-->");
                        if !parent_inline {
                            out.push('\n');
                        }
                    }
                    XmlNodeKind::CData(cdata) => {
                        if !parent_inline {
                            push_tabs(&mut out, depth);
                        }
                        write_cdata(&mut out, cdata);
                        if !parent_inline {
                            out.push('\n');
                        }
                    }
                    XmlNodeKind::Element {
                        name,
                        attrs,
                        children,
                        self_closing,
                    } => {
                        if name.is_empty() {
                            return Err(AlsError::InvalidFormat(
                                "empty XML element name".to_owned(),
                            ));
                        }
                        if !parent_inline {
                            push_tabs(&mut out, depth);
                        }
                        out.push('<');
                        out.push_str(name);
                        write_attrs(&mut out, attrs, mode);

                        if children.is_empty() {
                            if effective_self_closing(*self_closing, false) {
                                out.push_str(" />");
                            } else {
                                out.push_str("></");
                                out.push_str(name);
                                out.push('>');
                            }
                            if !parent_inline {
                                out.push('\n');
                            }
                        } else {
                            let children_inline = children.iter().any(|&cid| {
                                matches!(
                                    tree.node(cid).map(|n| &n.kind),
                                    Some(XmlNodeKind::Text(_))
                                )
                            }) || (children.len() == 1
                                && matches!(
                                    tree.node(children[0]).map(|n| &n.kind),
                                    Some(XmlNodeKind::CData(_))
                                ));
                            out.push('>');
                            if !children_inline {
                                out.push('\n');
                            }
                            stack.push(WriteOp::EndElement {
                                name,
                                depth,
                                indent_before_close: !children_inline,
                                newline_after_close: !parent_inline,
                            });
                            for &cid in children.iter().rev() {
                                stack.push(WriteOp::Node {
                                    id: cid,
                                    depth: depth + 1,
                                    parent_inline: children_inline,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(out)
}

/// Serializes `tree` in [`XmlFormatMode::LiveExact`] mode and writes it gzip-compressed to `path`.
pub fn write_als_gz(tree: &XmlTree, path: &Path) -> Result<(), AlsError> {
    let xml = write_xml_tree(tree, XmlFormatMode::LiveExact)?;
    let file = File::create(path)?;
    let mut encoder = GzEncoder::new(BufWriter::new(file), Compression::default());
    encoder.write_all(xml.as_bytes())?;
    encoder.finish()?;
    Ok(())
}

fn push_tabs(out: &mut String, count: usize) {
    for _ in 0..count {
        out.push('\t');
    }
}

fn write_attrs(out: &mut String, attrs: &[XmlAttr], mode: XmlFormatMode) {
    if attrs.is_empty() {
        return;
    }
    match mode {
        XmlFormatMode::LiveExact => {
            for attr in attrs {
                write_single_attr(out, attr);
            }
        }
        XmlFormatMode::PrettyCanonical => {
            if attrs.len() == 1 {
                write_single_attr(out, &attrs[0]);
                return;
            }
            let mut sorted: Vec<&XmlAttr> = attrs.iter().collect();
            sorted.sort_by(|a, b| a.name.cmp(&b.name));
            for attr in sorted {
                write_single_attr(out, attr);
            }
        }
    }
}

fn write_single_attr(out: &mut String, attr: &XmlAttr) {
    let quote = attr.effective_quote();
    let delim = match quote {
        QuoteStyle::Double => '"',
        QuoteStyle::Single => '\'',
    };
    out.push(' ');
    out.push_str(&attr.name);
    out.push('=');
    out.push(delim);
    escape_attr_value(out, &attr.value, quote);
    out.push(delim);
}

fn escape_attr_value(out: &mut String, value: &str, quote: QuoteStyle) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if quote == QuoteStyle::Double => out.push_str("&quot;"),
            '\'' if quote == QuoteStyle::Single => out.push_str("&apos;"),
            '\t' => out.push_str("&#9;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            _ => out.push(ch),
        }
    }
}

fn escape_text(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#13;"),
            _ => out.push(ch),
        }
    }
}

fn write_cdata(out: &mut String, cdata: &str) {
    let mut parts = cdata.split("]]>");
    if let Some(first) = parts.next() {
        out.push_str("<![CDATA[");
        out.push_str(first);
        for rest in parts {
            out.push_str("]]]]><![CDATA[>");
            out.push_str(rest);
        }
        out.push_str("]]>");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::als::read::{decompress_als_or_xml, read_xml_tree};
    use std::path::Path;

    #[test]
    fn byte_exact_roundtrip_on_livecraft_fixture() {
        let fixture = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<Ableton MajorVersion=\"5\" MinorVersion=\"12.0_12117\" Creator=\"LiveCraft 0.1.0\">\n",
            "\t<!-- LiveCraft fixture comment -->\n",
            "\t<LiveSet>\n",
            "\t\t<ViewData Value='{\"k\": [1,2]}' />\n",
            "\t\t<Envelopes></Envelopes>\n",
            "\t\t<Tag />\n",
            "\t\t<Escaped QuoteAttr=\"say &quot;hi&quot; &amp; 'ok'\" />\n",
            "\t\t<TextNode>5 &lt; 10 &amp; 20 &gt; 10</TextNode>\n",
            "\t\t<Blob><![CDATA[raw <xml> & data]]></Blob>\n",
            "\t</LiveSet>\n",
            "</Ableton>\n",
        );

        let tree = read_xml_tree(fixture).expect("parse fixture");
        let reserialized =
            write_xml_tree(&tree, XmlFormatMode::LiveExact).expect("serialize fixture");
        assert_eq!(reserialized, fixture);
        assert_eq!(reserialized.as_bytes(), fixture.as_bytes());
    }

    #[test]
    fn pretty_canonical_mode_sorts_attributes_stably() {
        let xml = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<Ableton Zebra=\"1\" Alpha=\"2\" Middle=\"3\" />\n",
        );
        let tree = read_xml_tree(xml).expect("parse");
        let pretty = write_xml_tree(&tree, XmlFormatMode::PrettyCanonical).expect("pretty");
        assert_eq!(
            pretty,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Ableton Alpha=\"2\" Middle=\"3\" Zebra=\"1\" />\n"
        );
    }

    #[test]
    fn write_als_gz_roundtrips_through_decompress() {
        let fixture = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<Ableton MajorVersion=\"5\">\n",
            "\t<LiveSet>\n",
            "\t\t<NextPointeeId Value=\"100\" />\n",
            "\t</LiveSet>\n",
            "</Ableton>\n",
        );
        let tree = read_xml_tree(fixture).expect("parse");
        let out_path = std::env::temp_dir().join("livecraft_m09_step_a_gz_roundtrip.als");
        write_als_gz(&tree, &out_path).expect("write gzip als");
        let loaded = decompress_als_or_xml(&out_path).expect("decompress written als");
        let _ = std::fs::remove_file(&out_path);
        assert_eq!(loaded.as_bytes(), fixture.as_bytes());
    }

    #[test]
    fn byte_exact_roundtrip_default_live_set_when_present() {
        let template_path =
            r"C:\ProgramData\Ableton\Live 12 Suite\Resources\Builtin\Templates\DefaultLiveSet.als";
        if !Path::new(template_path).exists() {
            return;
        }
        let decompressed = decompress_als_or_xml(template_path).expect("decompress DefaultLiveSet");
        let tree = read_xml_tree(&decompressed).expect("parse DefaultLiveSet XML");
        let reserialized =
            write_xml_tree(&tree, XmlFormatMode::LiveExact).expect("reserialize DefaultLiveSet");
        if reserialized.as_bytes() != decompressed.as_bytes() {
            let mismatch = decompressed
                .as_bytes()
                .iter()
                .zip(reserialized.as_bytes().iter())
                .position(|(a, b)| a != b)
                .unwrap_or_else(|| decompressed.len().min(reserialized.len()));
            let line = decompressed[..mismatch.min(decompressed.len())]
                .bytes()
                .filter(|&b| b == b'\n')
                .count()
                + 1;
            let orig_snippet =
                &decompressed[mismatch.saturating_sub(40)..(mismatch + 80).min(decompressed.len())];
            let new_snippet =
                &reserialized[mismatch.saturating_sub(40)..(mismatch + 80).min(reserialized.len())];
            panic!(
                "DefaultLiveSet.als round-trip byte mismatch at byte {mismatch} (line {line}):\n--- expected ---\n{orig_snippet}\n--- actual ---\n{new_snippet}"
            );
        }
        assert_eq!(reserialized.as_bytes(), decompressed.as_bytes());
    }

    #[test]
    fn all_livecraft_templates_parse_and_roundtrip_cleanly() {
        let templates: [(&str, &str); 9] = [
            (
                "empty_set.xml",
                include_str!("../../templates/empty_set.xml"),
            ),
            (
                "midi_track.xml",
                include_str!("../../templates/midi_track.xml"),
            ),
            (
                "audio_track.xml",
                include_str!("../../templates/audio_track.xml"),
            ),
            (
                "group_track.xml",
                include_str!("../../templates/group_track.xml"),
            ),
            (
                "return_track.xml",
                include_str!("../../templates/return_track.xml"),
            ),
            (
                "midi_clip.xml",
                include_str!("../../templates/midi_clip.xml"),
            ),
            (
                "audio_clip.xml",
                include_str!("../../templates/audio_clip.xml"),
            ),
            ("scene.xml", include_str!("../../templates/scene.xml")),
            (
                "automation_envelope.xml",
                include_str!("../../templates/automation_envelope.xml"),
            ),
        ];

        for (name, raw_xml) in templates {
            let tree = read_xml_tree(raw_xml)
                .unwrap_or_else(|e| panic!("template {name} failed to parse: {e}"));
            let reserialized = write_xml_tree(&tree, XmlFormatMode::LiveExact)
                .unwrap_or_else(|e| panic!("template {name} failed to write: {e}"));
            assert_eq!(
                reserialized, raw_xml,
                "template {name} did not round-trip byte-for-byte"
            );
        }
    }
}
