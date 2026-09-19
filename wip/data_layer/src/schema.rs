use rusqlite::{Connection, Error, Params, Result, Row, Statement};

static V1: &str = r"
CREATE TABLE kv (
  key TEXT PRIMARY KEY,
  value BLOB
);

CREATE TABLE entry (
  id INTEGER PRIMARY KEY,
  lexeme TEXT NOT NULL,
  gloss TEXT NOT NULL,
  pos TEXT,
  categories TEXT,
  notes TEXT,
  created DATETIME DEFAULT CURRENT_TIMESTAMP NOT NULL
);

INSERT INTO kv (key, value) VALUES ('version', 1);
";

#[derive(Debug)]
pub enum QueryError {
    SqlError(Error),
    UnexpectedType,
}

type QueryResult<T> = std::result::Result<T, QueryError>;

impl From<Error> for QueryError {
    fn from(value: Error) -> Self {
        Self::SqlError(value)
    }
}

#[derive(Debug, PartialEq)]
pub struct Entry {
    pub id: usize,
    pub lexeme: String,
    pub gloss: String,
    pub pos: String,
}

#[derive(Debug)]
pub struct DataLayer {
    conn: Connection,
}

impl DataLayer {
    pub fn init(conn: Connection) -> Result<DataLayer> {
        let layer = DataLayer { conn };
        let version = layer.get_version().unwrap_or(0);
        if version < 1 {
            layer.conn.execute_batch(V1)?;
        }
        Ok(layer)
    }

    pub fn get_version(&self) -> QueryResult<usize> {
        let value = "version";
        let mut stmt = self.conn.prepare("SELECT value FROM kv WHERE key = ?1")?;
        let version = stmt.query_row([&value], |row| {
            let value: usize = row.get(0)?;
            Ok(value)
        })?;
        Ok(version)
    }

    pub fn add_entry(&self, lexeme: &str, gloss: &str, pos: &str) -> QueryResult<usize> {
        self.conn
            .execute(
                "INSERT INTO entry (lexeme, gloss, pos) VALUES (?1, ?2, ?3)",
                (&lexeme, &gloss, &pos),
            )
            .map_err(|err| err.into())
    }

    pub fn has_lexeme(&self, lexeme: &str) -> QueryResult<bool> {
        let mut stmt = self
            .conn
            .prepare("SELECT COUNT(*) FROM entry WHERE lexeme = ?1 LIMIT 1")?;
        let has_lexeme = stmt.query_row([&lexeme], |row| {
            let count: usize = row.get(0)?;
            Ok(count > 0)
        })?;
        Ok(has_lexeme)
    }

    pub fn get_entries_by_lexeme(&self, lexeme: &str) -> QueryResult<Vec<Entry>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, lexeme, gloss, pos FROM entry WHERE lexeme = ?1")?;
        let entries = query_map_collect(&mut stmt, [&lexeme], |row| {
            Ok(Entry {
                id: row.get(0)?,
                lexeme: row.get(1)?,
                gloss: row.get(2)?,
                pos: row.get(3)?,
            })
        })?;
        Ok(entries)
    }
}

fn query_map_collect<T, P, F>(statement: &mut Statement, params: P, f: F) -> Result<Vec<T>>
where
    P: Params,
    F: FnMut(&Row<'_>) -> Result<T>,
{
    let mut vec = vec![];
    let iter = statement.query_map(params, f)?;
    for item in iter {
        let unwrapped_item = item?;
        vec.push(unwrapped_item);
    }
    Ok(vec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_empty() -> QueryResult<()> {
        rusqlite_regex::enable_auto_extension()?;
        let layer = DataLayer::init(Connection::open_in_memory()?)?;
        let result = layer.get_version()?;
        println!("{}", result);
        assert_eq!(result, 1);
        Ok(())
    }

    #[test]
    fn entries() -> QueryResult<()> {
        rusqlite_regex::enable_auto_extension()?;
        let layer = DataLayer::init(Connection::open_in_memory()?)?;
        layer.add_entry("foo", "bar", "verb")?;
        layer.add_entry("foo", "baz", "noun")?;
        layer.add_entry("other", "else", "adj")?;
        let entries = layer.get_entries_by_lexeme("foo")?;
        assert_eq!(
            entries,
            vec![
                Entry {
                    id: 1,
                    lexeme: "foo".into(),
                    gloss: "bar".into(),
                    pos: "verb".into()
                },
                Entry {
                    id: 2,
                    lexeme: "foo".into(),
                    gloss: "baz".into(),
                    pos: "noun".into()
                },
            ]
        );
        assert!(layer.has_lexeme("foo")?);
        assert!(!layer.has_lexeme("bar")?);
        Ok(())
    }
}
