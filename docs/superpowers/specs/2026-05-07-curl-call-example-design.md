# cURL Call Example Tab Design

Date: 2026-05-07

## Goal

Restore the old DBAPI request page workflow where an API can be tested and its call example can be viewed from separate tabs. The new implementation only needs cURL output, with a copy button.

## Current Context

The current React page is `frontend/src/pages/ApiRequestPage.tsx`. It already loads API detail, builds request parameters, sends the request, and shows the response.

The old Vue UI used `src/components/api/request.vue` plus `src/components/api/common/callExample.vue`. Its cURL output came from `src/utils/code/shell/index.js`. It did not use a cURL generation library; it hand-built the cURL string and used `vue-clipboard2` only for copying text.

`curlconverter.com` uses the open-source `curlconverter/curlconverter` project, but that project converts cURL into other languages. It is not the right fit for generating cURL from DBAPI's request form. `Kong/httpsnippet` can generate cURL from HAR and is a stronger candidate if DBAPI later needs many language snippets, but for this cURL-only feature it adds unnecessary conversion overhead.

## Scope

In scope:

- Add an `接口请求测试` / `调用示例` tab layout to `ApiRequestPage`.
- Keep the existing request test form and response behavior unchanged inside `接口请求测试`.
- Show a live cURL snippet inside `调用示例`.
- Add a `复制 cURL` button in the call example tab.
- Support DBAPI's existing POST-only user API calls.
- Support `application/json` with `--data-raw`.
- Support `application/x-www-form-urlencoded` with `--data-urlencode`.
- Support array params by emitting repeated `--data-urlencode` entries with the same parameter name.
- Include `Authorization` only when the current token field is non-empty.

Out of scope:

- Python, JavaScript, Go, Java, or other language snippets.
- Importing `curlconverter`, because it converts in the opposite direction.
- Importing `httpsnippet`, unless future requirements expand beyond cURL.
- Request methods other than POST.
- Full browser-devtools parity for every cURL flag.

## Design

Create a small frontend utility that accepts the same request state used by the send button:

- URL
- content type
- token
- JSON body text
- parsed form params

The utility returns a multi-line cURL string. It should quote shell arguments safely for common values containing spaces, quotes, newlines, or dollar signs. The page should call this utility during render so the call example updates as the user edits token, JSON, or form params.

The UI should use Ant Design `Tabs`, `Button`, and `Input.TextArea` or equivalent existing components. The cURL textarea should be read-only and large enough for multi-line commands. Copy should use `navigator.clipboard.writeText` with an Ant Design message on success or failure.

## Error Handling

If JSON body text is invalid, the cURL example should still show the raw text in `--data-raw` rather than blocking the tab. The existing send behavior can continue to validate JSON before sending.

If clipboard access fails, show a clear copy failure message and leave the cURL text visible.

## Tests

Add focused Vitest coverage for the cURL utility:

- JSON request with content type and raw JSON body.
- Form request with scalar params.
- Form request with array params as repeated `--data-urlencode`.
- Token present vs token absent.
- Shell quoting for single quotes and special characters.

Run:

```bash
npm run test -- --run
npm run build
```

