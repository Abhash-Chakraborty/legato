//! A formatted document: a title, headings, paragraphs and bullets made of
//! plain, bold or italic runs. Notes build one and write it out as HTML for
//! the clipboard or as a Word file, which Google Docs and Word open with
//! the same formatting. Nothing here goes online.

use std::io::Write;

use anyhow::Result;

/// Text in one style. A `\n` inside it is a line break.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
}

impl Run {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bold: false,
            italic: false,
        }
    }

    pub fn bold(text: impl Into<String>) -> Self {
        Self {
            bold: true,
            ..Self::plain(text)
        }
    }

    pub fn italic(text: impl Into<String>) -> Self {
        Self {
            italic: true,
            ..Self::plain(text)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Title(String),
    Heading(String),
    Para(Vec<Run>),
    Bullet(Vec<Run>),
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// HTML that Google Docs and Word keep the headings, lists and bold of when
/// it is pasted.
pub fn html(blocks: &[Block]) -> String {
    let runs = |runs: &[Run]| {
        runs.iter()
            .map(|r| {
                let mut text = esc(&r.text).replace('\n', "<br>");
                if r.italic {
                    text = format!("<i>{text}</i>");
                }
                if r.bold {
                    text = format!("<b>{text}</b>");
                }
                text
            })
            .collect::<String>()
    };
    let mut out = String::new();
    let mut in_list = false;
    for block in blocks {
        let bullet = matches!(block, Block::Bullet(_));
        if in_list && !bullet {
            out.push_str("</ul>");
        } else if !in_list && bullet {
            out.push_str("<ul>");
        }
        in_list = bullet;
        match block {
            Block::Title(text) => out.push_str(&format!("<h1>{}</h1>", esc(text))),
            Block::Heading(text) => out.push_str(&format!("<h2>{}</h2>", esc(text))),
            Block::Para(r) => out.push_str(&format!("<p>{}</p>", runs(r))),
            Block::Bullet(r) => out.push_str(&format!("<li>{}</li>", runs(r))),
        }
    }
    if in_list {
        out.push_str("</ul>");
    }
    out
}

/// Characters XML 1.0 can't hold, which a transcript could still contain.
fn xml_text(text: &str) -> String {
    esc(text)
        .chars()
        .filter(|&c| matches!(c, '\t' | '\n' | '\r') || c >= ' ')
        .collect()
}

fn docx_runs(runs: &[Run]) -> String {
    let mut out = String::new();
    for r in runs {
        let props = match (r.bold, r.italic) {
            (false, false) => String::new(),
            (b, i) => format!(
                "<w:rPr>{}{}</w:rPr>",
                if b { "<w:b/>" } else { "" },
                if i { "<w:i/>" } else { "" }
            ),
        };
        for (n, part) in r.text.split('\n').enumerate() {
            if n == 0 && part.is_empty() {
                continue;
            }
            out.push_str("<w:r>");
            out.push_str(&props);
            if n > 0 {
                out.push_str("<w:br/>");
            }
            if !part.is_empty() {
                out.push_str(&format!(
                    "<w:t xml:space=\"preserve\">{}</w:t>",
                    xml_text(part)
                ));
            }
            out.push_str("</w:r>");
        }
    }
    out
}

fn docx_body(blocks: &[Block]) -> String {
    let mut out = String::new();
    for block in blocks {
        let (style, runs) = match block {
            Block::Title(text) => ("Title", docx_runs(&[Run::plain(text.as_str())])),
            Block::Heading(text) => ("Heading1", docx_runs(&[Run::plain(text.as_str())])),
            Block::Para(r) => ("", docx_runs(r)),
            Block::Bullet(r) => ("ListBullet", docx_runs(r)),
        };
        out.push_str("<w:p>");
        if !style.is_empty() {
            out.push_str(&format!("<w:pPr><w:pStyle w:val=\"{style}\"/>"));
            if style == "ListBullet" {
                out.push_str("<w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"1\"/></w:numPr>");
            }
            out.push_str("</w:pPr>");
        }
        out.push_str(&runs);
        out.push_str("</w:p>");
    }
    out
}

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#;

const RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#;

const DOCUMENT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/></Relationships>"#;

/// Arial 11 like a new Google Doc, with the Title, Heading 1 and List
/// Bullet styles Docs maps to its own.
fn styles() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="{W}"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial" w:eastAsia="Arial" w:cs="Arial"/><w:sz w:val="22"/><w:szCs w:val="22"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="120" w:line="276" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:after="60"/></w:pPr><w:rPr><w:sz w:val="52"/><w:szCs w:val="52"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="360" w:after="120"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:sz w:val="32"/><w:szCs w:val="32"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="ListBullet"><w:name w:val="List Bullet"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="60"/></w:pPr></w:style></w:styles>"#
    )
}

fn numbering() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="{W}"><w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="singleLevel"/><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="•"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#
    )
}

fn core(title: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>{}</dc:title><dc:creator>Legato</dc:creator></cp:coreProperties>"#,
        xml_text(title)
    )
}

/// The document as a Word file (.docx).
pub fn docx(blocks: &[Block]) -> Result<Vec<u8>> {
    let title = blocks
        .iter()
        .find_map(|b| match b {
            Block::Title(t) => Some(t.as_str()),
            _ => None,
        })
        .unwrap_or_default();
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="{W}"><w:body>{}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#,
        docx_body(blocks)
    );
    let parts = [
        ("[Content_Types].xml", CONTENT_TYPES.to_string()),
        ("_rels/.rels", RELS.to_string()),
        ("docProps/core.xml", core(title)),
        ("word/_rels/document.xml.rels", DOCUMENT_RELS.to_string()),
        ("word/document.xml", document),
        ("word/styles.xml", styles()),
        ("word/numbering.xml", numbering()),
    ];
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, body) in parts {
        zip.start_file(name, options)?;
        zip.write_all(body.as_bytes())?;
    }
    Ok(zip.finish()?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn sample() -> Vec<Block> {
        vec![
            Block::Title("Infra <sync>".into()),
            Block::Para(vec![Run::italic("Oct 17 · Sahil")]),
            Block::Heading("Decisions".into()),
            Block::Bullet(vec![Run::bold("Ship"), Run::plain(": on Friday")]),
            Block::Bullet(vec![Run::plain("Rest")]),
            Block::Para(vec![Run::bold("Topic"), Run::plain("\nline two")]),
        ]
    }

    #[test]
    fn html_keeps_lists_breaks_and_escapes() {
        assert_eq!(
            html(&sample()),
            "<h1>Infra &lt;sync&gt;</h1><p><i>Oct 17 · Sahil</i></p><h2>Decisions</h2>\
             <ul><li><b>Ship</b>: on Friday</li><li>Rest</li></ul>\
             <p><b>Topic</b><br>line two</p>"
        );
        assert_eq!(
            html(&[Block::Bullet(vec![Run::plain("only")])]),
            "<ul><li>only</li></ul>"
        );
    }

    #[test]
    fn a_word_file_holds_the_parts_word_and_docs_need() {
        let bytes = docx(&sample()).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut names: Vec<String> = zip.file_names().map(String::from).collect();
        names.sort();
        assert_eq!(
            names,
            [
                "[Content_Types].xml",
                "_rels/.rels",
                "docProps/core.xml",
                "word/_rels/document.xml.rels",
                "word/document.xml",
                "word/numbering.xml",
                "word/styles.xml"
            ]
        );
        let mut doc = String::new();
        zip.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut doc)
            .unwrap();
        assert!(doc.contains(r#"<w:pStyle w:val="Title"/></w:pPr><w:r><w:t xml:space="preserve">Infra &lt;sync&gt;</w:t>"#));
        assert!(doc.contains(r#"<w:pStyle w:val="Heading1"/>"#));
        assert_eq!(doc.matches(r#"<w:numId w:val="1"/>"#).count(), 2);
        assert!(doc.contains(r#"<w:rPr><w:b/></w:rPr><w:t xml:space="preserve">Ship</w:t>"#));
        assert!(doc.contains(r#"<w:rPr><w:i/></w:rPr>"#));
        assert!(doc.contains("<w:br/><w:t xml:space=\"preserve\">line two</w:t>"));
        let mut core = String::new();
        zip.by_name("docProps/core.xml")
            .unwrap()
            .read_to_string(&mut core)
            .unwrap();
        assert!(core.contains("<dc:title>Infra &lt;sync&gt;</dc:title>"));
    }

    #[test]
    fn control_characters_never_reach_the_xml() {
        let bytes = docx(&[Block::Para(vec![Run::plain("a\u{1}b\u{7}c")])]).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut doc = String::new();
        zip.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut doc)
            .unwrap();
        assert!(doc.contains(">abc</w:t>"));
    }
}
