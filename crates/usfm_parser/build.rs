use std::env;
use std::fs::{File, read_to_string};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::str::FromStr;
use usfm_style::{StyleRule, StyleSheet, TextProperties};

const REF_STYLE: &str = r#"        StyleRule {
            marker: "ref".into(),
            name: Some("ref...ref* - Scripture Reference".into()),
            description: Some("A scripture reference".into()),
            style_type: StyleType::Character,
            text_type: TextType::NoteText,
            text_properties: TextProperties::from_bits(1168).unwrap(),
            nest: true,
            occurs_under: vec![],
        },"#;

/// An `Option<String>` as the Rust expression that rebuilds it.
fn option(value: &Option<String>) -> String {
    match value {
        Some(value) => format!("Some({value:?}.into())"),
        None => "None".to_string(),
    }
}

fn main() -> std::io::Result<()> {
    // Only re-run when an actual input changes.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=usfm.sty");
    println!("cargo:rerun-if-changed=usfm-extra.sty");

    // `usfm.sty` is Paratext's file, unmodified (NOTICE.md); the markers it
    // predates and the entries it gets wrong are corrected by appending
    // `usfm-extra.sty`, which the same parser reads. A repeated `\Marker`
    // amends the earlier entry field by field, so an entry there can add a
    // marker or change one line of an existing one.
    let mut sty = read_to_string("./usfm.sty")?;
    sty.push('\n');
    sty.push_str(&read_to_string("./usfm-extra.sty")?);
    let mut style_sheet = StyleSheet::from_str(&sty)
        .map_err(|e| std::io::Error::other(format!("failed to parse the stylesheet: {e:?}")))?;

    style_sheet.rules.push(StyleRule {
        marker: "flag".into(),
        name: None,
        description: None,
        style_type: usfm_style::StyleType::Character,
        text_properties: TextProperties::default(),
        text_type: usfm_style::TextType::Other,
        nest: true,
        occurs_under: vec![],
    });

    // Generated into OUT_DIR and pulled in with `include!` from `lib.rs`, so
    // the build never writes into `src/` and nothing generated is tracked.
    let out_dir = env::var("OUT_DIR").expect("cargo sets OUT_DIR for build scripts");
    let out = File::create(Path::new(&out_dir).join("default_stylesheet.rs"))?;
    let mut writer = BufWriter::new(out);
    writeln!(writer, "// GENERATED - DO NOT EDIT")?;
    writeln!(writer)?;
    writeln!(writer, "use std::sync::{{Arc, LazyLock}};")?;
    writeln!(
        writer,
        "use usfm_style::{{StyleSheet, StyleRule, StyleType, TextType, TextProperties}};"
    )?;
    writeln!(writer)?;
    // The stylesheet is handed out as an `Arc` so a `Document` can own it
    // (hardening plan D3) without copying ~1500 rules per parse.
    writeln!(
        writer,
        "pub static DEFAULT_STYLESHEET: LazyLock<Arc<StyleSheet>> = LazyLock::new(||"
    )?;
    writeln!(writer, "    Arc::new(StyleSheet::new(vec![")?;
    for rule in style_sheet.rules.iter() {
        writeln!(writer, "        StyleRule {{")?;
        writeln!(writer, "            marker: {:?}.into(),", rule.marker)?;
        // `\Name` and `\Description` are documentation the sheet carries and
        // the language server shows on hover (ticket 31); a rule the sheet
        // says nothing about writes `None` rather than an empty string.
        writeln!(writer, "            name: {},", option(&rule.name))?;
        writeln!(
            writer,
            "            description: {},",
            option(&rule.description)
        )?;
        writeln!(
            writer,
            "            style_type: StyleType::{:?},",
            rule.style_type
        )?;
        writeln!(
            writer,
            "            text_type: TextType::{:?},",
            rule.text_type
        )?;
        writeln!(
            writer,
            "            text_properties: TextProperties::from_bits({:?}).unwrap(),",
            rule.text_properties.bits()
        )?;
        writeln!(writer, "            nest: {},", rule.nest)?;
        writeln!(
            writer,
            "            occurs_under: {:?}.iter().map(|s: &&str| s.to_string()).collect(),",
            rule.occurs_under
        )?;
        writeln!(writer, "        }},")?;
    }
    writeln!(writer, "{}", REF_STYLE)?;
    writeln!(writer, "    ]))")?;
    writeln!(writer, ");")?;

    Ok(())
}
