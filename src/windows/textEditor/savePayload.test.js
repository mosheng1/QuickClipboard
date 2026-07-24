import test from 'node:test';
import assert from 'node:assert/strict';
import { resolveEditorSavePayload } from './savePayload.js';

const extractPlainTextFromHtml = (html) => `plain:${html}`;

test('clears stale HTML when plain text is the last edited representation', () => {
  const result = resolveEditorSavePayload({
    supportsHtml: true,
    textContent: 'new text',
    htmlContent: '<b>old HTML</b>',
    originalTextContent: 'old HTML',
    originalHtmlContent: '<b>old HTML</b>',
    lastEditedMode: 'text',
    extractPlainTextFromHtml,
  });

  assert.deepEqual(result, {
    contentForSave: 'new text',
    htmlPayload: '',
  });
});

test('derives plain text when HTML is edited', () => {
  const result = resolveEditorSavePayload({
    supportsHtml: true,
    textContent: 'old text',
    htmlContent: '<b>new HTML</b>',
    originalTextContent: 'old text',
    originalHtmlContent: '<b>old HTML</b>',
    lastEditedMode: 'html',
    extractPlainTextFromHtml,
  });

  assert.deepEqual(result, {
    contentForSave: 'plain:<b>new HTML</b>',
    htmlPayload: '<b>new HTML</b>',
  });
});

test('preserves unchanged HTML for non-content edits', () => {
  const result = resolveEditorSavePayload({
    supportsHtml: true,
    textContent: 'text',
    htmlContent: '<b>text</b>',
    originalTextContent: 'text',
    originalHtmlContent: '<b>text</b>',
    lastEditedMode: null,
    extractPlainTextFromHtml,
  });

  assert.deepEqual(result, {
    contentForSave: 'text',
    htmlPayload: '<b>text</b>',
  });
});
