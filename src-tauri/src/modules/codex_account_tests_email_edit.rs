use super::{
    find_existing_access_token_account_id, update_account_note, CodexAccountNoteUpdate,
};

fn at_only_account(id: &str, email: &str, token: &str) -> crate::models::codex::CodexAccount {
    crate::models::codex::CodexAccount::new(
        id.to_string(),
        email.to_string(),
        crate::models::codex::CodexTokens {
            id_token: String::new(),
            access_token: token.to_string(),
            refresh_token: None,
        },
    )
}

fn seed_index_summary(account: &crate::models::codex::CodexAccount) {
    let mut index = load_account_index();
    index.accounts.retain(|item| item.id != account.id);
    index.accounts.push(crate::models::codex::CodexAccountSummary {
        id: account.id.clone(),
        email: account.email.clone(),
        plan_type: account.plan_type.clone(),
        subscription_active_until: account.subscription_active_until.clone(),
        created_at: account.created_at,
        last_used: account.last_used,
    });
    save_account_index(&index).expect("seed index");
}

#[test]
fn email_edit_updates_account_and_index_for_at_only_account() {
    let _lock = crate::modules::test_support::env_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let _env = TestEnvGuard::new("codex-email-edit-at-only");
    let account = at_only_account(
        "codex-at-email-edit-1",
        "old@example.test",
        "at-email-edit-token",
    );
    save_account(&account).expect("save account");
    seed_index_summary(&account);

    let updated = update_account_note(
        &account.id,
        CodexAccountNoteUpdate {
            email: Some("new@example.test".to_string()),
            ..Default::default()
        },
        None,
    )
    .expect("update email");

    assert_eq!(updated.id, account.id);
    assert_eq!(updated.email, "new@example.test");
    assert_eq!(updated.tokens.access_token, "at-email-edit-token");
    assert_eq!(load_account(&account.id).expect("reload").email, "new@example.test");
    let index = load_account_index();
    assert!(index
        .accounts
        .iter()
        .any(|item| item.id == account.id && item.email == "new@example.test"));
}

#[test]
fn email_edit_rejects_oauth_and_non_token_accounts() {
    let _lock = crate::modules::test_support::env_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let _env = TestEnvGuard::new("codex-email-edit-reject");

    let mut oauth = at_only_account(
        "codex-at-email-edit-oauth",
        "oauth@example.test",
        "at-oauth-token",
    );
    oauth.tokens.refresh_token = Some("rt-token".to_string());
    save_account(&oauth).expect("save oauth account");
    let error = update_account_note(
        &oauth.id,
        CodexAccountNoteUpdate {
            email: Some("changed@example.test".to_string()),
            ..Default::default()
        },
        None,
    )
    .expect_err("oauth email must stay read-only");
    assert!(error.contains("仅 at-*"), "unexpected error: {error}");
    assert_eq!(load_account(&oauth.id).expect("reload").email, "oauth@example.test");

    let mut api_key = at_only_account(
        "codex-at-email-edit-api-key",
        "apikey@example.test",
        "at-api-key-token",
    );
    api_key.openai_api_key = Some("sk-test".to_string());
    save_account(&api_key).expect("save api key account");
    assert!(update_account_note(
        &api_key.id,
        CodexAccountNoteUpdate {
            email: Some("changed@example.test".to_string()),
            ..Default::default()
        },
        None,
    )
    .is_err());
}

#[test]
fn email_edit_rejects_invalid_values_without_writing() {
    let _lock = crate::modules::test_support::env_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let _env = TestEnvGuard::new("codex-email-edit-invalid");
    let account = at_only_account(
        "codex-at-email-edit-invalid",
        "keep@example.test",
        "at-invalid-token",
    );
    save_account(&account).expect("save account");

    for invalid in ["", "   ", "not-an-email", "a@b", "a@@b.test", "a@.test", "a@b..test"] {
        let error = update_account_note(
            &account.id,
            CodexAccountNoteUpdate {
                email: Some(invalid.to_string()),
                ..Default::default()
            },
            None,
        )
        .expect_err("invalid email must be rejected");
        assert!(!error.is_empty());
        assert_eq!(load_account(&account.id).expect("reload").email, "keep@example.test");
    }
}

#[test]
fn reimport_same_token_after_email_edit_reuses_existing_account() {
    let _lock = crate::modules::test_support::env_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let _env = TestEnvGuard::new("codex-email-edit-reimport");
    let token = "at-reimport-dedup-token";
    let account = at_only_account(
        "codex-at-reimport-dedup",
        "edited@example.test",
        token,
    );
    save_account(&account).expect("save account");
    seed_index_summary(&account);

    let matched = find_existing_access_token_account_id(&load_account_index(), token);
    assert_eq!(matched.as_deref(), Some(account.id.as_str()));

    let imported = upsert_account_from_access_token_with_hints(
        token.to_string(),
        CodexAccessTokenImportHints {
            email: Some("original@example.test".to_string()),
            ..Default::default()
        },
    )
    .expect("reimport");

    assert_eq!(imported.id, account.id);
    assert_eq!(imported.email, "edited@example.test");
    let index = load_account_index();
    assert_eq!(
        index
            .accounts
            .iter()
            .filter(|item| item.id == account.id)
            .count(),
        1
    );
}
