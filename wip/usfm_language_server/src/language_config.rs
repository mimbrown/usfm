use std::{
    collections::HashMap,
    ops::Deref,
    str::{Chars, FromStr},
};

use regex::{Error, Regex};
use serde::{Deserialize, Serialize};

mod regex_format {
    use regex::Regex;
    use serde::{self, Deserialize, Deserializer, Serializer};

    // The signature of a serialize_with function must follow the pattern:
    //
    //    fn serialize<S>(&T, S) -> Result<S::Ok, S::Error>
    //    where
    //        S: Serializer
    //
    // although it may also be generic over the input types T.
    pub fn serialize<S>(regex: &Regex, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{}", regex);
        serializer.serialize_str(&s)
    }

    // The signature of a deserialize_with function must follow the pattern:
    //
    //    fn deserialize<'de, D>(D) -> Result<T, D::Error>
    //    where
    //        D: Deserializer<'de>
    //
    // although it may also be generic over the output types T.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Regex, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.try_into().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LanguageConfiguration {
    categories: HashMap<String, HashMap<String, String>>,
    parts_of_speech: HashMap<String, PartOfSpeech>,
    rules: Vec<GenerativeRule>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PartOfSpeech {
    categories: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GenerativeRule {
    applies_to: Vec<String>,
    generate: Generator,
    generate_reverse: Generator,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Generator {
    #[serde(with = "regex_format")]
    replace: Regex,
    with: String,
}

impl Generator {
    fn apply(&self, word: &str) -> Option<String> {
        let replaced = self.replace.replace_all(word, &self.with);
        if replaced == word {
            None
        } else {
            Some(replaced.to_string())
        }
    }
}

// #[derive(Debug, Deserialize, Clone)]
// #[serde(rename_all = "camelCase")]
// pub struct GeneratedOutput {
//   replace: Vec<Matcher>,
//   categories: HashMap<String, Option<String>>,
// }

// struct RegexGroups<'a> {
//   text: &'a str,
//   chars: Chars<'a>,
//   cursor: usize,
//   level: usize,
// }

// impl<'a> RegexGroups<'a> {
//   fn new(text: &'a str) -> RegexGroups {
//     RegexGroups {
//       text,
//       chars: text.chars(),
//       cursor: 0,
//       level: 0,
//     }
//   }
// }

// impl<'a> Iterator for RegexGroups<'a> {
//   type Item = &'a str;

//   fn next(&mut self) -> Option<Self::Item> {
//     let mut start: Option<(usize, usize)> = None;
//     let mut end: Option<usize> = None;
//     let mut reset: Option<(usize, usize)> = None;
//     while let Some(ch) = self.chars.next() {
//       match ch {
//         '\\' => {
//           self.cursor += ch.len_utf8();
//           if let Some(ch) = self.chars.next() {
//             self.cursor += ch.len_utf8();
//           }
//         },
//         '(' => {
//           let cursor = self.cursor;
//           let level = self.level;
//           self.cursor += ch.len_utf8();
//           self.level += 1;
//           let is_capturing = match self.text[self.cursor..].chars().next() {
//             Some('?') => false,
//             Some(_) => true,
//             None => false,
//           };
//           if start.is_some() {
//             if is_capturing && reset.is_none() {
//               reset = Some((cursor, level));
//             }
//           } else if is_capturing {
//             start = Some((cursor, level));
//           }
//         },
//         ')' => {
//           self.cursor += ch.len_utf8();
//           self.level -= 1;
//           if let Some((_, start_level)) = start {
//             if start_level == self.level {
//               end = Some(self.cursor);
//               break;
//             }
//           }
//         },
//         _ => {
//           self.cursor += ch.len_utf8();
//         }
//       }
//     }
//     if let Some((cursor, level)) = reset {
//       self.cursor = cursor;
//       self.level = level;
//       self.chars = self.text[cursor..].chars();
//     }
//     if let (Some((start, _)), Some(end)) = (start, end) {
//       Some(&self.text[start..end])
//     } else {
//       None
//     }
//   }
// }

// fn reverse(original: &str, replace: &str) -> Result<Regex, Error> {
//   let mut pattern = String::new();
//   if original.starts_with('^') {
//     pattern.push('^');
//   }
//   let mut chars = replace.chars();
//   while let Some(ch) = chars.next() {
//     if ch == '$' {
//       let Some(next) = chars.next() else {
//         return Err(Error::Syntax("Unexpected end of string".into()));
//       };
//       match next {
//         '1'..='9' => {
//           let group = next.to_string().parse::<usize>().unwrap();
//           let Some(group) = RegexGroups::new(original).into_iter().skip(group - 1).next() else {
//             return Err(Error::Syntax("Reference capture group not found".into()));
//           };
//           pattern.push_str(group);
//         },
//         '&' => {
//           pattern.push_str("(?:");
//           pattern.push_str(original);
//           pattern.push(')');
//         },
//         _ => return Err(Error::Syntax("Expected capture group to be 1-9".to_string()))
//       };
//     } else {
//       pattern.push(ch);
//     }
//   }
//   if original.ends_with('$') {
//     pattern.push('$');
//   }

//   Regex::new(&pattern)
// }

// #[derive(Debug, Clone)]
// pub struct Matcher(Regex);

// impl<'de> Deserialize<'de> for Matcher {
//   fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//   where
//     D: serde::Deserializer<'de> {
//     let s = String::deserialize(deserializer)?;
//     Ok(s.parse().map_err(serde::de::Error::custom)?)
//   }
// }

// impl Deref for Matcher {
//   type Target = Regex;

//   fn deref(&self) -> &Self::Target {
//     &self.0
//   }
// }

// impl core::str::FromStr for Matcher {
//   type Err = Error;

//   /// Attempts to parse a string into a regular expression
//   fn from_str(s: &str) -> Result<Matcher, Error> {
//     let (s, prefix) = match s.strip_prefix('-') {
//       Some(s) => (s, true),
//       None => (s, false),
//     };
//     let (s, suffix) = match s.strip_suffix('-') {
//       Some(s) => (s, true),
//       None => (s, false),
//     };
//     let s = format!("{}{s}{}", if prefix { "" } else { "^" }, if suffix { "" } else { "$" });
//     Ok(Matcher(Regex::new(&s)?))
//   }
// }

pub struct Language {
    map_category: HashMap<String, CategoryType>,
    config: LanguageConfiguration,
}

impl Language {
    pub fn from_config(config: LanguageConfiguration) -> Language {
        let mut map_category = HashMap::new();
        for (category, options) in config.categories.iter() {
            for (option, name) in options {
                map_category.insert(
                    option.clone(),
                    CategoryType {
                        category: category.clone(),
                        name: name.clone(),
                    },
                );
            }
        }
        Language {
            map_category,
            config,
        }
    }

    pub fn parse_categories(&self, categories: &str) -> Result<HashMap<String, String>, String> {
        let mut map = HashMap::new();
        for category in categories.split('.') {
            if let Some(category_type) = self.map_category.get(category) {
                if map
                    .insert(category_type.category.clone(), category_type.name.clone())
                    .is_some()
                {
                    return Err("Duplicate category".into());
                }
            } else {
                return Err("Unknown category".into());
            }
        }
        Ok(map)
    }

    pub fn get_possibilities(&self, word: &str) -> Vec<String> {
        let mut possibilities = vec![];
        for rule in self.config.rules.iter() {
            if let Some(possibility) = rule.generate_reverse.apply(word) {
                possibilities.push(possibility);
            }
        }
        possibilities
    }
}

pub struct CategoryType {
    category: String,
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = r#"
{
  "categories": {
    "gender": {
      "m": "masculine",
      "f": "feminine"
    },
    "pov": {
      "1": "first person",
      "2": "second person",
      "3": "third person"
    },
    "number": {
      "sg": "singular",
      "pl": "plural"
    },
    "case": {
      "nom": "nominative",
      "obl": "oblique",
      "voc": "vocative",
      "dat": "dative"
    }
  },
  "characterClasses": {
    "vowels": "əɛɪɔʊaeiou",
    "longVowels": "aeiou",
    "shortVowels": "əɛɪɔʊ"
  },
  "transforms": {
    "lax": {
      "a": "ə",
      "e": "ɛ",
      "i": "ɪ",
      "o": "ɔ",
      "u": "ʊ"
    }
  },
  "partsOfSpeech": {
    "pronoun": {
      "categories": ["case", "gender", "pov", "number"]
    },
    "noun": {
      "categories": ["gender", "number"]
    }
  },
  "rules": [
    {
      "appliesTo": ["pronoun", "noun"],
      "generate": { "replace": "o$", "with": "i" },
      "generateReverse": { "replace": "i$", "with": "o" },
      "categories": {
        "gender": "f"
      }
    },
    {
      "appliesTo": ["pronoun", "noun"],
      "generate": { "replace": "o$", "with": "a" },
      "generateReverse": { "replace": "a$", "with": "o" },
      "categories": {
        "gender": null,
        "number": "pl"
      }
    }
  ]
}
"#;

    #[test]
    fn parse_category() {
        let language = Language::from_config(serde_json::de::from_str(CONFIG).unwrap());
        assert_eq!(
            language.parse_categories("f.dat"),
            Ok(HashMap::from_iter([
                ("gender".into(), "feminine".into()),
                ("case".into(), "dative".into()),
            ]))
        );
        assert!(language.parse_categories("f.m").is_err());
        assert!(language.parse_categories("f.foo").is_err());
    }

    #[test]
    fn get_categories() {
        let language = Language::from_config(serde_json::de::from_str(CONFIG).unwrap());
        assert_eq!(language.get_possibilities("meri"), vec!["mero"]);
        assert_eq!(language.get_possibilities("mera"), vec!["mero"]);
        assert!(language.get_possibilities("mero").is_empty());
    }

    // #[test]
    // fn get_capture_groups() {
    //   let groups: Vec<_> = RegexGroups::new("test(?:(group 1 (?:foo) (group 2))) (group 3)").into_iter().collect();
    //   assert_eq!(groups, vec![
    //     "(group 1 (?:foo) (group 2))",
    //     "(group 2)",
    //     "(group 3)",
    //   ]);
    // }

    // #[test]
    // fn matcher() {
    //   let matcher: Matcher = "-o".parse().unwrap();
    //   assert!(matcher.is_match("go"));
    //   assert!(!matcher.is_match("og"));
    // }

    // #[test]
    // fn reverse_regex() {
    //   assert_eq!(reverse(".*", "$&o").unwrap().as_str(), "(?:.*)o$");
    //   assert_eq!(reverse(".*o$", "$&e").unwrap().as_str(), "(?:.*o$)e$");
    //   assert_eq!(reverse("(.*)o$", "e$1").unwrap().as_str(), "e(.*)$");
    //   assert_eq!(reverse("o$", "i").unwrap().as_str(), "i$");
    // }
}
