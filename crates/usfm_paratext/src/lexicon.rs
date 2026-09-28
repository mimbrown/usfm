//! `Lexicon.xml`: the project's lexicon, whose senses gloss the clusters of
//! its interlinear files (ticket 56).
//!
//! ```xml
//! <Lexicon>
//!   <Entries>
//!     <item>
//!       <Lexeme Type="Word" Form="wug" Homograph="1" />
//!       <Entry><Sense Id="s-wug1"><Gloss Language="es">uno</Gloss></Sense></Entry>
//!     </item>
//!   </Entries>
//! </Lexicon>
//! ```
//!
//! Read by the rules of SIL's `pt9-xml.md` (see [`crate::interlinear`]): an
//! entry keeps the last of a repeated key, a missing `Type` is `Phrase`, an
//! unknown or empty one, a missing `Form` or a non-numeric `Homograph` fails
//! the file. `Language`, `FontName` and `FontSize` are left out: Paratext
//! overwrites them from the project's settings when it loads the file.

use std::collections::HashMap;

use crate::interlinear::{ClusterLexeme, LexemeKey, LexemeType, child};

/// A gloss of a sense, in one language.
#[derive(Debug, Clone, PartialEq)]
pub struct Gloss {
    /// A language tag, or a language name in older files.
    pub language: Option<String>,
    pub text: String,
}

/// One meaning of an entry. The interlinear files point at it by `id`.
#[derive(Debug, Clone, PartialEq)]
pub struct Sense {
    /// `None` for a sense no cluster can point at.
    pub id: Option<String>,
    pub glosses: Vec<Gloss>,
}

impl Sense {
    /// The gloss in `language`, compared ignoring ASCII case (a file may say
    /// `es-MX` where another says `es-mx`).
    pub fn gloss(&self, language: &str) -> Option<&str> {
        self.glosses
            .iter()
            .find(|g| {
                g.language
                    .as_deref()
                    .is_some_and(|l| l.eq_ignore_ascii_case(language))
            })
            .map(|g| g.text.as_str())
    }
}

/// A lexeme and its senses (none, commonly, for a morpheme).
#[derive(Debug, Clone, PartialEq)]
pub struct LexiconEntry {
    pub key: LexemeKey,
    pub senses: Vec<Sense>,
}

/// A wordform and the morphemes it divides into, from the legacy `Analyses`
/// section projects untouched since Paratext 8 still carry.
#[derive(Debug, Clone, PartialEq)]
pub struct WordAnalysis {
    pub word: String,
    pub lexemes: Vec<LexemeKey>,
}

/// One `Lexicon.xml`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Lexicon {
    entries: Vec<LexiconEntry>,
    analyses: Vec<WordAnalysis>,
    /// Sense id -> (entry, sense). The first of a repeated id wins.
    senses: HashMap<String, (usize, usize)>,
}

impl Lexicon {
    pub fn from_xml(xml: &str) -> Result<Self, String> {
        let document = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
        let root = document.root_element();
        if !root.has_tag_name("Lexicon") {
            return Err(format!(
                "expected <Lexicon>, found <{}>",
                root.tag_name().name()
            ));
        }
        let mut lexicon = Self::default();
        for item in items(root, "Entries") {
            let key = read_key(child(item, "Lexeme").ok_or("an entry has no <Lexeme> key")?)?;
            let senses = child(item, "Entry")
                .into_iter()
                .flat_map(|entry| entry.children().filter(|n| n.has_tag_name("Sense")))
                .map(|sense| Sense {
                    id: sense.attribute("Id").map(str::to_string),
                    glosses: sense
                        .children()
                        .filter(|n| n.has_tag_name("Gloss"))
                        .map(|gloss| Gloss {
                            language: gloss.attribute("Language").map(str::to_string),
                            text: gloss.text().unwrap_or_default().to_string(),
                        })
                        .collect(),
                })
                .collect();
            let entry = LexiconEntry { key, senses };
            match lexicon.entries.iter_mut().find(|e| e.key == entry.key) {
                Some(earlier) => *earlier = entry,
                None => lexicon.entries.push(entry),
            }
        }
        for item in items(root, "Analyses") {
            let word = child(item, "string")
                .ok_or("an analysis has no <string> wordform")?
                .text()
                .unwrap_or_default()
                .to_string();
            let lexemes = child(item, "ArrayOfLexeme")
                .into_iter()
                .flat_map(|array| array.children().filter(|n| n.has_tag_name("Lexeme")))
                .map(read_key)
                .collect::<Result<Vec<_>, _>>()?;
            if lexemes.is_empty() {
                continue;
            }
            let analysis = WordAnalysis { word, lexemes };
            match lexicon
                .analyses
                .iter_mut()
                .find(|a| a.word == analysis.word)
            {
                Some(earlier) => *earlier = analysis,
                None => lexicon.analyses.push(analysis),
            }
        }
        for (e, entry) in lexicon.entries.iter().enumerate() {
            for (s, sense) in entry.senses.iter().enumerate() {
                if let Some(id) = &sense.id {
                    lexicon.senses.entry(id.clone()).or_insert((e, s));
                }
            }
        }
        Ok(lexicon)
    }

    /// Every entry, in file order.
    pub fn entries(&self) -> &[LexiconEntry] {
        &self.entries
    }

    pub fn entry(&self, key: &LexemeKey) -> Option<&LexiconEntry> {
        self.entries.iter().find(|entry| &entry.key == key)
    }

    /// The sense a cluster's `GlossId` names.
    pub fn sense(&self, id: &str) -> Option<&Sense> {
        let &(entry, sense) = self.senses.get(id)?;
        Some(&self.entries[entry].senses[sense])
    }

    /// A cluster lexeme's gloss in `language`: its chosen sense's, or, when
    /// it chose none, the first sense of its entry that has one — what
    /// Paratext shows for an unapproved guess.
    pub fn gloss(&self, lexeme: &ClusterLexeme, language: &str) -> Option<&str> {
        if let Some(id) = &lexeme.sense {
            return self.sense(id)?.gloss(language);
        }
        self.entry(&lexeme.key()?)?
            .senses
            .iter()
            .find_map(|sense| sense.gloss(language))
    }

    /// The legacy word analyses.
    pub fn analyses(&self) -> &[WordAnalysis] {
        &self.analyses
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.analyses.is_empty()
    }
}

fn items<'a, 'input>(
    root: roxmltree::Node<'a, 'input>,
    section: &'static str,
) -> impl Iterator<Item = roxmltree::Node<'a, 'input>> {
    root.children()
        .filter(move |n| n.has_tag_name(section))
        .flat_map(|s| s.children().filter(|n| n.has_tag_name("item")))
}

/// A `<Lexeme Type=".." Form=".." Homograph=".." />` key.
fn read_key(node: roxmltree::Node<'_, '_>) -> Result<LexemeKey, String> {
    let kind = match node.attribute("Type") {
        None => LexemeType::Phrase,
        Some(name) => match LexemeType::from_name(name) {
            LexemeType::Other(name) => return Err(format!("unknown lexeme type {name:?}")),
            kind => kind,
        },
    };
    let form = node
        .attribute("Form")
        .ok_or("a <Lexeme> key has no Form")?
        .to_string();
    let homograph = match node.attribute("Homograph") {
        None => 1,
        Some(value) => value
            .trim()
            .parse()
            .map_err(|_| format!("Homograph=\"{value}\" is not a number"))?,
    };
    Ok(LexemeKey {
        kind,
        form,
        homograph,
    })
}
