//! On-demand `.docx` OpenXML ZIP archive inspection and macOS desktop file integration
//! (`Preview`, `Microsoft Word`, and `Finder` reveal).

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::process::Command;

use zip::ZipArchive;

/// Inspectable source / IR / OpenXML document targets in the right-hand Inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorDoc {
    TypstSource,
    IrJson,
    DocumentXml,
    SettingsXml,
    StylesXml,
    NumberingXml,
    FootnotesXml,
    HeaderXml,
    FooterXml,
    DocumentRelsXml,
}

impl InspectorDoc {
    pub const ALL: [Self; 10] = [
        Self::TypstSource,
        Self::IrJson,
        Self::DocumentXml,
        Self::SettingsXml,
        Self::StylesXml,
        Self::NumberingXml,
        Self::FootnotesXml,
        Self::HeaderXml,
        Self::FooterXml,
        Self::DocumentRelsXml,
    ];

    /// Compact label displayed inside the Inspector dropdown button trigger.
    pub fn short_label(self) -> &'static str {
        match self {
            Self::TypstSource => "Typst (.typ)",
            Self::IrJson => "Semantic IR (ir.json)",
            Self::DocumentXml => "word/document.xml",
            Self::SettingsXml => "word/settings.xml",
            Self::StylesXml => "word/styles.xml",
            Self::NumberingXml => "word/numbering.xml",
            Self::FootnotesXml => "word/footnotes.xml",
            Self::HeaderXml => "word/header1.xml",
            Self::FooterXml => "word/footer1.xml",
            Self::DocumentRelsXml => "document.xml.rels",
        }
    }

    /// Full descriptive title shown inside the dropdown menu list.
    pub fn menu_label(self) -> &'static str {
        match self {
            Self::TypstSource => "Typst Source (.typ)   ⌘1",
            Self::IrJson => "Semantic IR (ir.json)   ⌘2",
            Self::DocumentXml => "Word Body (word/document.xml)   ⌘3",
            Self::SettingsXml => "Word Settings (word/settings.xml)   ⌘4",
            Self::StylesXml => "Word Styles (word/styles.xml)",
            Self::NumberingXml => "Word Numbering (word/numbering.xml)",
            Self::FootnotesXml => "Word Footnotes (word/footnotes.xml)",
            Self::HeaderXml => "Word Header (word/header1.xml)",
            Self::FooterXml => "Word Footer (word/footer1.xml)",
            Self::DocumentRelsXml => "Package Rels (word/_rels/document.xml.rels)",
        }
    }

    /// ZIP entry path inside `output.docx` if this target is an OpenXML package part.
    pub fn docx_entry_name(self) -> Option<&'static str> {
        match self {
            Self::TypstSource | Self::IrJson => None,
            Self::DocumentXml => Some("word/document.xml"),
            Self::SettingsXml => Some("word/settings.xml"),
            Self::StylesXml => Some("word/styles.xml"),
            Self::NumberingXml => Some("word/numbering.xml"),
            Self::FootnotesXml => Some("word/footnotes.xml"),
            Self::HeaderXml => Some("word/header1.xml"),
            Self::FooterXml => Some("word/footer1.xml"),
            Self::DocumentRelsXml => Some("word/_rels/document.xml.rels"),
        }
    }

    pub fn is_xml(self) -> bool {
        self.docx_entry_name().is_some()
    }
}

/// Discover which XML entry names exist inside `docx_path` so optional parts
/// (`numbering.xml`, `footnotes.xml`, `header1.xml`, `footer1.xml`) can be badged/enabled.
pub fn list_docx_entries(docx_path: &str) -> HashSet<String> {
    let mut entries = HashSet::new();
    if docx_path.is_empty() {
        return entries;
    }
    let Ok(file) = File::open(docx_path) else {
        return entries;
    };
    let Ok(mut zip) = ZipArchive::new(file) else {
        return entries;
    };
    for idx in 0..zip.len() {
        if let Ok(entry) = zip.by_index(idx)
            && let Some(path) = entry.enclosed_name()
            && let Some(name) = path.to_str()
        {
            entries.insert(name.replace('\\', "/"));
        }
    }
    entries
}

/// Read an XML entry from `output.docx` (falling back to cached `document.xml` on disk when applicable).
pub fn read_docx_xml_part(
    docx_path: &str,
    fallback_doc_xml_path: &str,
    doc: InspectorDoc,
) -> String {
    let Some(entry_name) = doc.docx_entry_name() else {
        return String::new();
    };

    if !docx_path.is_empty()
        && let Ok(file) = File::open(docx_path)
        && let Ok(mut zip) = ZipArchive::new(file)
        && let Ok(mut entry) = zip.by_name(entry_name)
    {
        let mut buf = String::new();
        if entry.read_to_string(&mut buf).is_ok() {
            return buf;
        }
    }

    if doc == InspectorDoc::DocumentXml
        && !fallback_doc_xml_path.is_empty()
        && let Ok(xml) = fs::read_to_string(fallback_doc_xml_path)
    {
        return xml;
    }

    if docx_path.is_empty() || !Path::new(docx_path).exists() {
        format!(
            "<!-- `{entry_name}` not generated yet — click ▶ Run (⌘R) to compile this case -->\n"
        )
    } else {
        format!("<!-- Optional part `{entry_name}` is not present in this .docx package -->\n")
    }
}

/// Open a `.docx` file in Microsoft Word on macOS (explicit user-requested inspection).
pub fn open_file_in_word(path: &str) -> Result<(), String> {
    if path.is_empty() || !Path::new(path).exists() {
        return Err("output.docx does not exist yet — run the case first".to_string());
    }
    Command::new("open")
        .args(["-a", "Microsoft Word", path])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("failed to open in Word: {e}"))
}

/// Open a PDF or image in macOS Preview.
pub fn open_file_in_preview(path: &str) -> Result<(), String> {
    if path.is_empty() || !Path::new(path).exists() {
        return Err("File does not exist on disk yet — run the case first".to_string());
    }
    Command::new("open")
        .args(["-a", "Preview", path])
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("failed to open in Preview: {e}"))
}

/// Reveal a file or case directory in macOS Finder.
pub fn reveal_in_finder(path: &str) -> Result<(), String> {
    if path.is_empty() || !Path::new(path).exists() {
        return Err("Case artifacts directory not created yet — run the case first".to_string());
    }
    let p = Path::new(path);
    let mut cmd = Command::new("open");
    if p.is_file() {
        cmd.arg("-R").arg(path);
    } else {
        cmd.arg(path);
    }
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("failed to reveal in Finder: {e}"))
}
