// Cloudflare Worker — policy-gateway 公网节点
// 与路由器 Rust 后端共享同一套 API，权限表存 KV。
// Phase 0: /api/signup + /api/signup/status + /manager

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    // CORS 预检
    if (request.method === 'OPTIONS') {
      return new Response(null, {
        headers: { 'Access-Control-Allow-Origin': '*', 'Access-Control-Allow-Methods': 'POST,GET,OPTIONS' }
      });
    }

    try {
      switch (true) {
        case request.method === 'POST' && url.pathname === '/api/signup':
          return handleSignup(request, env);
        case request.method === 'GET' && url.pathname === '/api/signup/status':
          return handleStatus(request, url, env);
        case request.method === 'GET' && url.pathname === '/manager':
          return handleManager(request, env);
        case request.method === 'POST' && url.pathname === '/api/manager/approve':
          return handleApprove(request, env);
        case request.method === 'GET' && url.pathname === '/sync/pull':
          return handleSyncPull(env);
        case request.method === 'POST' && url.pathname === '/sync/push':
          return handleSyncPush(request, env);
        default:
          return new Response('policy-gateway Worker\n', { status: 404 });
      }
    } catch (e) {
      return new Response(JSON.stringify({ error: e.message }), {
        status: 500,
        headers: { 'Content-Type': 'application/json' }
      });
    }
  }
};

/// POST /api/signup — 提交证书 + 主机名
async function handleSignup(request, env) {
  const { cert, hostname } = await request.json();

  if (!cert || !hostname) {
    return json({ error: 'cert 和 hostname 必填' }, 400);
  }

  // 计算 SHA256
  const encoder = new TextEncoder();
  const certBytes = encoder.encode(cert);
  const hashBuffer = await crypto.subtle.digest('SHA-256', certBytes);
  const sha256 = bytesToHex(new Uint8Array(hashBuffer));

  // 查重
  const existing = await env.AUTH_TABLE.get(sha256);
  if (existing) {
    return json({ error: '证书已存在' }, 409);
  }

  // 存入 KV
  const requestId = crypto.randomUUID();
  const entry = JSON.stringify({
    sha256, hostname,
    bitmap: 0,
    status: 'pending',
    requestId,
    createdAt: Date.now()
  });

  await env.AUTH_TABLE.put(sha256, entry);
  await env.PENDING_QUEUE.put(requestId, sha256);

  return json({ request_id: requestId, status: 'pending', sha256 });
}

/// GET /api/signup/status?id=xxx&sha256=xxx
async function handleStatus(request, url, env) {
  const id = url.searchParams.get('id');
  const sha256 = url.searchParams.get('sha256');

  let key = sha256;
  if (!key && id) {
    key = await env.PENDING_QUEUE.get(id);
  }
  if (!key) {
    return json({ status: 'not_found' });
  }

  const raw = await env.AUTH_TABLE.get(key);
  if (!raw) {
    return json({ status: 'not_found' });
  }

  const entry = JSON.parse(raw);
  return json({
    status: entry.status,
    hostname: entry.hostname,
    bitmap: entry.bitmap ? BigInt(entry.bitmap).toString(16) : null,
  });
}

/// GET /manager — 简易 HTML 审批面板
async function handleManager(request, env) {
  let rows = '';
  const list = await env.AUTH_TABLE.list();

  for (const key of list.keys) {
    const raw = await env.AUTH_TABLE.get(key.name);
    if (!raw) continue;
    const e = JSON.parse(raw);
    if (e.status !== 'pending') continue;

    rows += `<tr>
      <td>${e.hostname}</td>
      <td><code>${e.sha256.slice(0, 16)}...</code></td>
      <td>${e.status}</td>
      <td>
        <form action="/api/manager/approve" method="post" style="display:inline">
          <input type="hidden" name="requestId" value="${e.requestId}" />
          <input type="hidden" name="action" value="approve" />
          <label>bitmap: <input name="bitmap" value="01" size="4" /></label>
          <button type="submit">✅ 同意</button>
        </form>
      </td>
    </tr>`;
  }

  const html = `<!DOCTYPE html>
<html lang="zh">
<head><meta charset="UTF-8"><title>Worker 审批面板</title>
<style>body{font-family:sans-serif;max-width:800px;margin:auto;padding:20px}
table{width:100%;border-collapse:collapse}
td,th{border:1px solid #ddd;padding:8px}
button{cursor:pointer}</style>
</head>
<body>
<h1>☁️ Worker 审批面板</h1>
<table><tr><th>主机名</th><th>SHA256</th><th>状态</th><th>操作</th></tr>${rows}</table>
<p><small>bitmap: 01=connector</small></p>
</body>
</html>`;
  return new Response(html, { headers: { 'Content-Type': 'text/html; charset=utf-8' } });
}

/// POST /api/manager/approve
async function handleApprove(request, env) {
  const body = await request.json();
  const { requestId, action, bitmap } = body;

  const sha256 = await env.PENDING_QUEUE.get(requestId);
  if (!sha256) return json({ error: 'request_id 不存在' }, 404);

  const raw = await env.AUTH_TABLE.get(sha256);
  if (!raw) return json({ error: '证书不存在' }, 404);

  const entry = JSON.parse(raw);
  entry.status = action === 'approve' ? 'active' : 'rejected';
  if (bitmap) entry.bitmap = bitmap;

  await env.AUTH_TABLE.put(sha256, JSON.stringify(entry));
  await env.PENDING_QUEUE.delete(requestId);

  return json({ status: entry.status, message: `已${action === 'approve' ? '批准' : '拒绝'}` });
}

/// GET /sync/pull — 路由器拉取 Worker 数据
async function handleSyncPull(env) {
  const entries = [];
  const list = await env.AUTH_TABLE.list();
  for (const key of list.keys) {
    const raw = await env.AUTH_TABLE.get(key.name);
    if (raw) entries.push(JSON.parse(raw));
  }
  return json({ version: Date.now(), entries });
}

/// POST /sync/push — 路由器推送数据到 Worker
async function handleSyncPush(request, env) {
  const packet = await request.json();
  for (const entry of packet.entries) {
    await env.AUTH_TABLE.put(entry.sha256, JSON.stringify(entry));
  }
  return json({ status: 'synced', count: packet.entries.length });
}

// --- 工具函数 ---

function json(data, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: { 'Content-Type': 'application/json' }
  });
}

function bytesToHex(bytes) {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
}
