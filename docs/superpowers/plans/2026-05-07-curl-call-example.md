# cURL Call Example Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore the request page call-example tab with live cURL output and a copy cURL button.

**Architecture:** Keep the page behavior in `ApiRequestPage.tsx`, but move cURL generation into a focused utility under `frontend/src/components`. The utility takes normalized request state and returns a shell-safe multi-line cURL command; the page renders that command in a read-only tab.

**Tech Stack:** React 19, Ant Design 6, TypeScript, Vitest, Vite.

---

## File Structure

- Create `frontend/src/components/curlExample.ts`: cURL generation helper and shell quoting.
- Create `frontend/src/components/curlExample.test.ts`: focused Vitest coverage for JSON, form, arrays, auth, and quoting.
- Modify `frontend/src/pages/ApiRequestPage.tsx`: add `Tabs`, live cURL generation, read-only cURL display, and `复制 cURL`.

---

### Task 1: cURL Generator Tests

**Files:**
- Create: `frontend/src/components/curlExample.test.ts`

- [x] **Step 1: Write the failing tests**

Create `frontend/src/components/curlExample.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { generateCurlCommand } from './curlExample';

describe('generateCurlCommand', () => {
  it('generates JSON cURL with content type and Authorization', () => {
    const curl = generateCurlCommand({
      url: 'http://127.0.0.1:8520/api/student/all',
      contentType: 'application/json',
      token: 'abc123',
      body: '{"id":23}',
    });

    expect(curl).toBe(
      [
        "curl -X POST 'http://127.0.0.1:8520/api/student/all' \\",
        "  -H 'Content-Type: application/json' \\",
        "  -H 'Authorization: abc123' \\",
        "  --data-raw '{\"id\":23}'",
      ].join('\n'),
    );
  });

  it('omits Authorization when token is blank', () => {
    const curl = generateCurlCommand({
      url: 'http://127.0.0.1:8520/api/student/all',
      contentType: 'application/json',
      token: '   ',
      body: '{}',
    });

    expect(curl).not.toContain('Authorization');
  });

  it('generates form cURL with scalar params', () => {
    const curl = generateCurlCommand({
      url: 'http://127.0.0.1:8520/api/student/all',
      contentType: 'application/x-www-form-urlencoded',
      params: [
        { name: 'id', value: 23 },
        { name: 'name', value: 'aaa' },
      ],
    });

    expect(curl).toBe(
      [
        "curl -X POST 'http://127.0.0.1:8520/api/student/all' \\",
        "  -H 'Content-Type: application/x-www-form-urlencoded' \\",
        "  --data-urlencode 'id=23' \\",
        "  --data-urlencode 'name=aaa'",
      ].join('\n'),
    );
  });

  it('generates repeated form params for arrays', () => {
    const curl = generateCurlCommand({
      url: 'http://127.0.0.1:8520/api/student/all',
      contentType: 'application/x-www-form-urlencoded',
      params: [
        {
          name: 'ids',
          values: [{ va: 1 }, { va: 2 }],
        },
      ],
    });

    expect(curl).toContain("--data-urlencode 'ids=1' \\");
    expect(curl).toContain("--data-urlencode 'ids=2'");
  });

  it('shell-quotes single quotes and dollar signs safely', () => {
    const curl = generateCurlCommand({
      url: "http://127.0.0.1:8520/api/student/o'clock",
      contentType: 'application/json',
      token: 'sk-$demo',
      body: '{"name":"o\'clock","price":"$1"}',
    });

    expect(curl).toContain("'http://127.0.0.1:8520/api/student/o'\\''clock'");
    expect(curl).toContain("-H 'Authorization: sk-$demo'");
    expect(curl).toContain("--data-raw '{\"name\":\"o'\\''clock\",\"price\":\"$1\"}'");
  });
});
```

- [x] **Step 2: Run the focused test to verify it fails**

Run:

```bash
cd frontend
npm run test -- --run src/components/curlExample.test.ts
```

Expected: FAIL because `./curlExample` does not exist.

- [x] **Step 3: Commit the failing tests**

```bash
git add frontend/src/components/curlExample.test.ts
git commit -m "test: cover curl example generation"
```

---

### Task 2: cURL Generator Implementation

**Files:**
- Create: `frontend/src/components/curlExample.ts`
- Test: `frontend/src/components/curlExample.test.ts`

- [x] **Step 1: Implement the generator**

Create `frontend/src/components/curlExample.ts`:

```ts
export interface CurlFormParam {
  name: string;
  value?: unknown;
  values?: Array<{ va: unknown }>;
}

export interface CurlCommandInput {
  url: string;
  contentType: string;
  token?: string;
  body?: string;
  params?: CurlFormParam[];
}

export function generateCurlCommand(input: CurlCommandInput): string {
  const lines = [`curl -X POST ${shellQuote(input.url)}`];
  lines.push(`  -H ${shellQuote(`Content-Type: ${input.contentType}`)}`);

  const token = input.token?.trim();
  if (token) {
    lines.push(`  -H ${shellQuote(`Authorization: ${token}`)}`);
  }

  if (input.contentType.startsWith('application/x-www-form-urlencoded')) {
    for (const part of formParts(input.params ?? [])) {
      lines.push(`  --data-urlencode ${shellQuote(part)}`);
    }
  } else {
    lines.push(`  --data-raw ${shellQuote(input.body ?? '{}')}`);
  }

  return lines.map((line, index) => (index < lines.length - 1 ? `${line} \\` : line)).join('\n');
}

function formParts(params: CurlFormParam[]): string[] {
  const parts: string[] = [];
  for (const param of params) {
    if (!param.name) continue;

    if (param.values?.length) {
      for (const item of param.values) {
        parts.push(`${param.name}=${stringValue(item.va)}`);
      }
      continue;
    }

    parts.push(`${param.name}=${stringValue(param.value)}`);
  }
  return parts;
}

function stringValue(value: unknown): string {
  if (value === null || value === undefined) return '';
  return String(value);
}

function shellQuote(value: string): string {
  return `'${value.replace(/'/g, `'\\''`)}'`;
}
```

- [x] **Step 2: Run the focused test**

Run:

```bash
cd frontend
npm run test -- --run src/components/curlExample.test.ts
```

Expected: PASS.

- [x] **Step 3: Commit the implementation**

```bash
git add frontend/src/components/curlExample.ts frontend/src/components/curlExample.test.ts
git commit -m "feat: add curl example generator"
```

---

### Task 3: Request Page Tab UI

**Files:**
- Modify: `frontend/src/pages/ApiRequestPage.tsx`
- Uses: `frontend/src/components/curlExample.ts`

- [x] **Step 1: Update imports**

In `frontend/src/pages/ApiRequestPage.tsx`, change imports to include `CopyOutlined`, `Tabs`, and `generateCurlCommand`:

```ts
import { CopyOutlined, DeleteOutlined, PlusOutlined, SendOutlined } from '@ant-design/icons';
import { App, Button, Card, Empty, Form, Input, Space, Tabs, Tag, Typography } from 'antd';
import { generateCurlCommand } from '../components/curlExample';
```

- [x] **Step 2: Add URL and cURL derived values**

Inside `ApiRequestPage`, after `const isJson = ...`, add:

```ts
  const requestUrl = `http://${address}/api/${detail?.path || ''}`.replace('/api//', '/api/');
  const curlCommand = generateCurlCommand({
    url: requestUrl,
    contentType,
    token,
    body: isJson ? jsonBody || '{}' : undefined,
    params: isJson ? undefined : params,
  });
```

- [x] **Step 3: Add copy handler**

Inside `ApiRequestPage`, before `async function send()`, add:

```ts
  async function copyCurl() {
    try {
      await navigator.clipboard.writeText(curlCommand);
      message.success('cURL 已复制');
    } catch (error) {
      message.error(error instanceof Error ? error.message : '复制 cURL 失败');
    }
  }
```

- [x] **Step 4: Wrap request form and cURL example in tabs**

Replace the current returned `<div className="mx-auto max-w-5xl space-y-4">...</div>` body with a layout that keeps the existing form in the first tab and adds the cURL textarea in the second tab:

```tsx
  return (
    <div className="mx-auto max-w-5xl space-y-4">
      <Typography.Title level={3}>{detail?.name || '请求测试'}</Typography.Title>
      <Tabs
        items={[
          {
            key: 'request',
            label: '接口请求测试',
            children: (
              <>
                <Card>
                  <Form layout="vertical">
                    <Form.Item label="URL">
                      <Input value={requestUrl} readOnly />
                    </Form.Item>
                    <Form.Item label="Content-Type">
                      <Input value={contentType} readOnly />
                    </Form.Item>
                    <Form.Item label="Authorization">
                      <Input value={token} onChange={(event) => setToken(event.target.value)} />
                    </Form.Item>

                    <Form.Item label={isJson ? '请求参数 JSON' : '请求参数'}>
                      {/* Keep the existing JSON/form param editor JSX unchanged here. */}
                    </Form.Item>
                    <Button type="primary" icon={<SendOutlined />} onClick={send}>
                      发送请求
                    </Button>
                  </Form>
                </Card>
                <Card title="返回结果" className="mt-4">
                  <Input.TextArea rows={14} value={result} readOnly />
                </Card>
              </>
            ),
          },
          {
            key: 'curl',
            label: '调用示例',
            children: (
              <Card
                title="cURL"
                extra={
                  <Button icon={<CopyOutlined />} onClick={copyCurl}>
                    复制 cURL
                  </Button>
                }
              >
                <Input.TextArea rows={12} value={curlCommand} readOnly />
              </Card>
            ),
          },
        ]}
      />
    </div>
  );
```

When applying this step, move the existing JSON/form param editor JSX into the indicated location exactly as it exists today rather than rewriting it.

- [x] **Step 5: Run TypeScript build**

Run:

```bash
cd frontend
npm run build
```

Expected: PASS.

- [x] **Step 6: Commit the UI integration**

```bash
git add frontend/src/pages/ApiRequestPage.tsx
git commit -m "feat: restore curl call example tab"
```

---

### Task 4: Full Verification

**Files:**
- Verify all changed frontend files.

- [x] **Step 1: Run all frontend tests**

Run:

```bash
cd frontend
npm run test -- --run
```

Expected: PASS.

- [x] **Step 2: Run frontend build**

Run:

```bash
cd frontend
npm run build
```

Expected: PASS.

- [x] **Step 3: Check git status**

Run:

```bash
git status --short --branch
```

Expected: only known unrelated local files may remain, such as `data.db-shm`; all cURL feature files are committed.

- [x] **Step 4: Push main**

Run:

```bash
git push origin main
```

Expected: push succeeds to `origin/main`.

---

## Self-Review

Spec coverage:

- `接口请求测试` / `调用示例` tabs are covered in Task 3.
- cURL-only scope is covered in Tasks 1-3.
- JSON, form, arrays, auth, and quoting are covered in Task 1.
- Copy button is covered in Task 3.
- Verification commands are covered in Task 4.

No placeholders remain. The only intentionally high-level instruction is to move existing JSX unchanged inside the new tab structure so current request editing behavior is preserved without duplicating the entire existing component body in this plan.
