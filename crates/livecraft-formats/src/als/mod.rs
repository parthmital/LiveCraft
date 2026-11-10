//! Ableton Live Set (`.als`) lossless XML DOM, mappers, diffing, and [`AlsParser`] facade.

pub mod diff;
pub mod map;
pub mod read;
pub mod tree;
pub mod write;

use std::io::Read;
use std::path::Path;

use livecraft_model::model::Project;

pub use diff::{LiveSetDiff, diff_als_files, diff_projects, format_diff_summary};
pub use map::AlsDocument;

/// Errors raised while reading, parsing, modifying, or writing `.als` Live Sets.
#[derive(Debug, thiserror::Error)]
pub enum AlsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("XML parsing error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("Decompression failed: {0}")]
    Decompress(String),
    #[error("Invalid Ableton file format: {0}")]
    InvalidFormat(String),
    #[error("Decompressed Live Set is larger than the {limit}-byte limit")]
    TooLarge { limit: u64 },
}

/// Backwards-compatible parser/writer facade backed by [`AlsDocument`] and [`tree::XmlTree`].
pub struct AlsParser;

impl AlsParser {
    /// Largest decompressed `.als` XML accepted, in bytes (512 MiB).
    pub const MAX_XML_BYTES: u64 = read::MAX_XML_BYTES;

    /// Decompresses a `.als` file (or reads plain UTF-8 `.xml`) into an XML string.
    pub fn decompress_to_xml<P: AsRef<Path>>(path: P) -> Result<String, AlsError> {
        read::decompress_als_or_xml(path)
    }

    /// Decompresses gzip data (or plain UTF-8) from `reader` into UTF-8 text of at most `limit` bytes.
    pub fn decompress_reader<R: Read>(reader: R, limit: u64) -> Result<String, AlsError> {
        read::decompress_reader(reader, limit)
    }

    /// Loads a `.als` (or `.xml`) file directly into a LiveCraft [`Project`].
    pub fn load_project<P: AsRef<Path>>(path: P) -> Result<Project, AlsError> {
        Ok(AlsDocument::open(path)?.into_project())
    }

    /// Parses an XML string into a LiveCraft [`Project`].
    pub fn parse_xml(xml: &str, project_name: &str) -> Result<Project, AlsError> {
        Ok(AlsDocument::from_xml_named(xml, project_name)?.into_project())
    }

    /// Saves a LiveCraft [`Project`] as a gzip-compressed Ableton Live `.als` file.
    pub fn save_project_to_als<P: AsRef<Path>>(project: &Project, path: P) -> Result<(), AlsError> {
        AlsDocument::from_project(project)?.save_als(path)
    }

    /// Exports a LiveCraft [`Project`] to an Ableton-compatible XML string (`LiveExact`).
    pub fn export_to_xml(project: &Project) -> Result<String, AlsError> {
        AlsDocument::from_project(project)?.to_xml()
    }

    /// Exports a LiveCraft [`Project`] to canonical pretty-printed XML with sorted attributes.
    pub fn export_to_pretty_xml(project: &Project) -> Result<String, AlsError> {
        AlsDocument::from_project(project)?.to_pretty_xml()
    }
}
