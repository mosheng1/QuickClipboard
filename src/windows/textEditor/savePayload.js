export function resolveEditorSavePayload({
  supportsHtml,
  textContent,
  htmlContent,
  originalTextContent,
  originalHtmlContent,
  lastEditedMode,
  extractPlainTextFromHtml,
}) {
  if (!supportsHtml) {
    return {
      contentForSave: textContent,
      htmlPayload: undefined,
    };
  }

  const textChanged = textContent !== originalTextContent;
  const htmlChanged = htmlContent !== originalHtmlContent;

  // Plain-text edits cannot be safely merged back into the previous HTML.
  // Clear the stale HTML so formatted paste uses the newly edited text.
  if (lastEditedMode === 'text' && textChanged) {
    return {
      contentForSave: textContent,
      htmlPayload: '',
    };
  }

  if (htmlChanged) {
    return {
      contentForSave: extractPlainTextFromHtml(htmlContent),
      htmlPayload: htmlContent,
    };
  }

  return {
    contentForSave: textContent,
    htmlPayload: htmlContent,
  };
}
