//! Single-device public setup hints. No owner credentials, inventory API or writes.
use crate::network::Ticket;
use serde::Deserialize;
use std::time::Duration;

const ENDPOINT: &str = "https://droidfun.io/api/droids/identify";

pub fn valid_id(id: &str) -> bool {
    id.len() == 16 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Hint {
    model: Option<String>,
}

fn model(bytes: &[u8]) -> Option<String> {
    let hint: Hint = serde_json::from_slice(bytes).ok()?;
    hint.model
        .filter(|m| crate::onboarding::FACTORY_MODELS.contains(&m.as_str()))
}

pub async fn lookup(id: &str, ticket: Ticket) -> Option<String> {
    if !valid_id(id) || !ticket.current() {
        return None;
    }
    let client = reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(1))
        .timeout(Duration::from_secs(2))
        .build()
        .ok()?;
    let request = async {
        if !ticket.current() {
            return None;
        }
        let mut response = client
            .post(ENDPOINT)
            .json(&serde_json::json!({"boardId": id}))
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            if bytes.len() + chunk.len() > 256 {
                return None;
            }
            bytes.extend_from_slice(&chunk);
        }
        ticket.current().then(|| model(&bytes)).flatten()
    };
    let canceled = async {
        while ticket.current() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    tokio::select! { biased; _ = canceled => None, result = request => result }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_supported_minimal_hints_and_hardware_ids_are_accepted() {
        assert!(valid_id("0123456789ABCDEF"));
        for id in [
            "",
            "BOARD_A",
            "../admin",
            "0123456789ABCDEG",
            "0123456789ABCDEF0",
        ] {
            assert!(!valid_id(id));
        }
        for name in crate::onboarding::FACTORY_MODELS {
            assert_eq!(
                model(format!(r#"{{"model":"{name}"}}"#).as_bytes()).as_deref(),
                Some(name)
            );
        }
        for bytes in [
            br#"{"model":"other"}"#.as_slice(),
            br#"{"model":"boo","note":"private"}"#,
            br#"{"model":null}"#,
            br#"[]"#,
        ] {
            assert!(model(bytes).is_none());
        }
    }
    #[tokio::test]
    async fn canceled_setup_never_issues_a_lookup() {
        let policy = crate::network::Policy::new("", false);
        let ticket = policy.ticket("droid_setup").unwrap();
        policy.update("", true);
        assert!(lookup("0123456789ABCDEF", ticket).await.is_none());
        assert!(policy.ticket("droid_setup").is_err());
    }
}
