use serde::{Deserialize, Deserializer, Serialize, de::Error};
use serde_json::Value;
use tower_lsp_server::lsp_types::Uri;

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum Run {
    OnSave,
    #[default]
    OnType,
}

#[derive(Debug, Default, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    pub run: Run,
    pub config_path: String,
}

impl<'de> Deserialize<'de> for Options {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        Options::try_from(value).map_err(Error::custom)
    }
}

impl TryFrom<Value> for Options {
    type Error = String;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let Some(object) = value.as_object() else {
            return Err("no object passed".to_string());
        };

        Ok(Self {
            run: object
                .get("run")
                .map(|run| serde_json::from_value::<Run>(run.clone()).unwrap_or_default())
                .unwrap_or_default(),
            config_path: object
                .get("configPath")
                .and_then(|config_path| serde_json::from_value::<String>(config_path.clone()).ok())
                .unwrap_or("usfm.config.json".into()),
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceOption {
    pub workspace_uri: Uri,
    pub options: Options,
}

#[cfg(test)]
mod test {
    use serde_json::json;

    use super::{Options, Run, WorkspaceOption};

    #[test]
    fn test_valid_options_json() {
        let json = json!({
            "run": "onSave",
            "configPath": "./custom.json",
        });

        let options = Options::try_from(json).unwrap();
        assert_eq!(options.run, Run::OnSave);
        assert_eq!(options.config_path, "./custom.json".to_string());
    }

    #[test]
    fn test_empty_options_json() {
        let json = json!({});

        let options = Options::try_from(json).unwrap();
        assert_eq!(options.run, Run::OnType);
        assert_eq!(options.config_path, "usfm.config.json".to_string());
    }

    #[test]
    fn test_invalid_options_json() {
        let json = json!({
            "run": true,
            "configPath": "./custom.json"
        });

        let options = Options::try_from(json).unwrap();
        assert_eq!(options.run, Run::OnType); // fallback
        assert_eq!(options.config_path, "./custom.json".to_string());
    }

    #[test]
    fn test_invalid_workspace_options_json() {
        let json = json!([{
            "workspaceUri": "file:///root/",
            "options": {
                "run": true,
                "configPath": "./custom.json"
            }
        }]);

        let workspace = serde_json::from_value::<Vec<WorkspaceOption>>(json).unwrap();

        assert_eq!(workspace.len(), 1);
        assert_eq!(workspace[0].workspace_uri.path().as_str(), "/root/");

        let options = &workspace[0].options;
        assert_eq!(options.run, Run::OnType); // fallback
        assert_eq!(options.config_path, "./custom.json".to_string());
    }
}
