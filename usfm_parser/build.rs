use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use usfm_style::{StyleRule, StyleSheet, TextProperties};

const REF_STYLE: &str = r#"        StyleRule {
            marker: "ref".into(),
            style_type: StyleType::Character,
            text_type: TextType::NoteText,
            text_properties: TextProperties::from_bits(1168).unwrap(),
            nest: true,
            occurs_under: vec![],
        },"#;

fn main() -> std::io::Result<()> {
    // Only re-run when an actual input changes.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=usfm.sty");

    let mut style_sheet = StyleSheet::from_file("./usfm.sty")?;

    style_sheet.rules.push(StyleRule {
        marker: "flag".into(),
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
    writeln!(writer, "")?;
    writeln!(writer, "use std::sync::{{Arc, LazyLock}};")?;
    writeln!(
        writer,
        "use usfm_style::{{StyleSheet, StyleRule, StyleType, TextType, TextProperties}};"
    )?;
    writeln!(writer, "")?;
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
