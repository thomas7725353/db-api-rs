import { describe, expect, it } from 'vitest';
import { generateCurlCommand } from './curlExample';

describe('generateCurlCommand', () => {
  it('generates GET curl with query string parameters', () => {
    const curl = generateCurlCommand({
      method: 'GET',
      url: 'http://127.0.0.1:8520/api/demo/items/list',
      contentType: 'application/json',
      token: 'sk-demo',
      params: [
        { name: 'limit', value: 10 },
        { name: 'offset', value: 0 },
      ],
    });

    expect(curl).toBe(
      [
        "curl 'http://127.0.0.1:8520/api/demo/items/list?limit=10&offset=0' \\",
        "  -H 'Authorization: sk-demo'",
      ].join('\n'),
    );
  });

  it('generates DELETE curl with query string parameters and no body', () => {
    const curl = generateCurlCommand({
      method: 'DELETE',
      url: 'http://127.0.0.1:8520/api/demo/items/delete',
      contentType: 'application/json',
      params: [{ name: 'id', value: 1 }],
    });

    expect(curl).toBe("curl -X DELETE 'http://127.0.0.1:8520/api/demo/items/delete?id=1'");
  });

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
