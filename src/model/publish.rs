//! View models for publish service output (US1–US3).

use serde::Serialize;

use crate::model::index::PublishRecord;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "target_type", rename_all = "kebab-case")]
pub enum PublishRunOutcome {
    Local(LocalRunOutcome),
    #[serde(rename = "yuque-prompt")]
    YuquePrompt(YuquePromptRunOutcome),
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalRunOutcome {
    pub target_name: String,
    pub article: String,
    pub source: String,
    pub destination: String,
    pub size_bytes: u64,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct YuquePromptRunOutcome {
    pub target_name: String,
    pub article: String,
    pub source_path: String,
    pub build_artifact_path: String,
    pub content: String,
    pub prompt: String,
    pub envelope: serde_json::Value,
    pub suggested_update_command: String,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublishUpdateOutcome {
    pub article: String,
    pub target_name: String,
    pub action: UpdateAction,
    pub record: PublishRecord,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateAction {
    Created,
    Updated,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::index::PublishStatus;

    #[test]
    fn local_outcome_target_type_is_local() {
        let outcome = PublishRunOutcome::Local(LocalRunOutcome {
            target_name: "tgt".to_string(),
            article: "a".to_string(),
            source: "/a".to_string(),
            destination: "/b".to_string(),
            size_bytes: 12,
            dry_run: false,
        });
        let v = serde_json::to_value(&outcome).unwrap();
        assert_eq!(v["target_type"], "local");
        assert_eq!(v["dry_run"], false);
    }

    #[test]
    fn yuque_prompt_outcome_target_type_is_kebab_case() {
        let outcome = PublishRunOutcome::YuquePrompt(YuquePromptRunOutcome {
            target_name: "tgt".to_string(),
            article: "a".to_string(),
            source_path: "docs/a.md".to_string(),
            build_artifact_path: "/p/_build/a.md".to_string(),
            content: "hello".to_string(),
            prompt: "Please publish ...".to_string(),
            envelope: serde_json::json!({}),
            suggested_update_command: "mf publish update a --target tgt --status published --target-url <URL>"
                .to_string(),
            dry_run: true,
        });
        let v = serde_json::to_value(&outcome).unwrap();
        assert_eq!(v["target_type"], "yuque-prompt");
        assert_eq!(v["dry_run"], true);
        assert_eq!(v["envelope"], serde_json::json!({}));
    }

    #[test]
    fn update_action_serializes_snake_case() {
        assert_eq!(
            serde_json::to_value(UpdateAction::Created).unwrap(),
            serde_json::Value::String("created".to_string())
        );
        assert_eq!(
            serde_json::to_value(UpdateAction::Updated).unwrap(),
            serde_json::Value::String("updated".to_string())
        );
    }

    #[test]
    fn update_outcome_includes_record_with_null_published_at() {
        let outcome = PublishUpdateOutcome {
            article: "a".to_string(),
            target_name: "tgt".to_string(),
            action: UpdateAction::Created,
            record: PublishRecord {
                path: "docs/a.md".to_string(),
                target_name: "tgt".to_string(),
                status: PublishStatus::Draft,
                target_url: None,
                published_at: None,
            },
            dry_run: false,
        };
        let v = serde_json::to_value(&outcome).unwrap();
        assert_eq!(v["action"], "created");
        assert_eq!(v["record"]["status"], "draft");
        assert_eq!(v["record"]["published_at"], serde_json::Value::Null);
        assert_eq!(v["record"]["target_url"], serde_json::Value::Null);
    }
}
