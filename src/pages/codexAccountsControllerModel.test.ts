import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  buildCodexAccountNoteForm,
  EMPTY_CODEX_ACCOUNT_NOTE_FORM,
} from './codexAccountsControllerModel';
import type { CodexAccount } from '../types/codex';

test('account note form seeds the email label from the account', () => {
  const account = { email: 'current@example.test' } as CodexAccount;
  assert.equal(buildCodexAccountNoteForm(account).email, 'current@example.test');
  assert.equal(buildCodexAccountNoteForm(null).email, '');
  assert.equal(EMPTY_CODEX_ACCOUNT_NOTE_FORM.email, '');
});
