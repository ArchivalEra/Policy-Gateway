// vm-mod-worker — Cloudflare Worker 镜像模块
// 提供与路由器一致的 /manager + /signup + /permissions
//
// 部署: 作为 Cloudflare Pages Function 或独立 Worker
// 依赖: core Worker 的 /recover 用于根证书恢复
// 同步: 通过 /sync/pull + /sync/push 与路由器同步权限表

// === Module endpoints ===
async function handleSignup(request, env) {
  const url = new URL(request.url);
  if (request.method === 'GET') {
    return serveStatic('signup.html', env);
  }
  // POST /api/signup — 接收 CSR/pubkey，存 pending
  // 路由器上线后自动签名
  const { csr, pubkey, hostname, requested, hw_platform } = await request.json();
  if (!hostname) return json({ error: 'hostname required' }, 400);
  // 生成 request_id，存 KV
  const requestId = crypto.randomUUID();
  const sha256 = 'pending_' + requestId; // 待路由器签名后更新
  const entry = { requestId, hostname, csr: csr || null, pubkey: pubkey || null,
    requested: requested || '01', hw_platform: hw_platform || null,
    status: 'pending_router_sign', created_at: Date.now() };
  await env.PENDING_QUEUE.put(requestId, JSON.stringify(entry));
  return json({ request_id: requestId, status: 'pending_router_sign', sha256 });
}

async function handleStatus(request, env) {
  const url = new URL(request.url);
  const id = url.searchParams.get('id');
  const sha256 = url.searchParams.get('sha256');
  if (sha256) {
    const raw = await env.AUTH_TABLE.get(sha256);
    if (raw) return json(JSON.parse(raw));
    return json({ status: 'not_found' });
  }
  if (id) {
    const raw = await env.PENDING_QUEUE.get(id);
    if (raw) return json(JSON.parse(raw));
    return json({ status: 'not_found' });
  }
  return json({ status: 'missing_query' }, 400);
}

async function handleManager(request, env) {
  const url = new URL(request.url);
  const token = url.searchParams.get('token') || '';
  const MANAGER_TOKEN = env.MANAGER_TOKEN || '';
  if (token !== MANAGER_TOKEN) {
    if (request.method === 'GET') {
      return new Response('Unauthorized', { status: 401 });
    }
    return json({ error: 'unauthorized' }, 401);
  }
  if (request.method === 'GET') {
    // 列出 pending 条目
    const pending = [];
    const list = await env.PENDING_QUEUE.list();
    for (const key of list.keys) {
      const raw = await env.PENDING_QUEUE.get(key.name);
      if (raw) pending.push(JSON.parse(raw));
    }
    return json({ count: pending.length, entries: pending });
  }
  // POST /api/manager/approve
  const { request_id, action } = await request.json();
  if (action === 'approve') {
    const raw = await env.PENDING_QUEUE.get(request_id);
    if (!raw) return json({ error: 'not found' }, 404);
    // 标记为 approved → 路由器上线后签名
    const entry = JSON.parse(raw);
    entry.status = 'approved';
    await env.PENDING_QUEUE.put(request_id, JSON.stringify(entry));
    return json({ status: 'approved', message: '已批准，待路由器签名' });
  }
  if (action === 'reject') {
    await env.PENDING_QUEUE.delete(request_id);
    return json({ status: 'rejected', message: '已拒绝' });
  }
  return json({ error: 'unknown action' }, 400);
}

// === Sync endpoints ===
async function handleSyncPull(env) {
  // 返回所有权限表条目
  const entries = [];
  const list = await env.AUTH_TABLE.list();
  for (const key of list.keys) {
    const raw = await env.AUTH_TABLE.get(key.name);
    if (raw) entries.push({ sha256: key.name, entry: JSON.parse(raw) });
  }
  return json({ count: entries.length, entries });
}

async function handleSyncPush(request, env) {
  const { entries } = await request.json();
  let count = 0;
  for (const { sha256, entry } of entries || []) {
    await env.AUTH_TABLE.put(sha256, JSON.stringify(entry));
    count++;
  }
  return json({ status: 'ok', synced: count });
}

// === Static file serving ===
function serveStatic(path, env) {
  // 从 Cloudflare Pages 静态资源加载
  // 如果部署为 Pages Function，请求会自动匹配静态文件
  // 这里返回 404 表示由 Pages 静态资源处理
  return new Response('Not Found (static file should be served by Pages)', { status: 404 });
}

// === Router ===
export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (request.method === 'OPTIONS') {
      return new Response(null, { headers: { 'Access-Control-Allow-Origin': '*',
        'Access-Control-Allow-Methods': 'GET,POST,OPTIONS' } });
    }
    try {
      // Signup
      if (url.pathname === '/signup' || url.pathname === '/api/signup')
        return handleSignup(request, env);
      // Status
      if (url.pathname === '/signup/status' || url.pathname === '/api/signup/status')
        return handleStatus(request, env);
      // Manager
      if (url.pathname === '/manager' || url.pathname === '/api/manager/approve'
          || url.pathname === '/api/manager/pending')
        return handleManager(request, env);
      // Sync
      if (url.pathname === '/sync/pull') return handleSyncPull(env);
      if (url.pathname === '/sync/push') return handleSyncPush(request, env);
      // Core recovery (delegate to core Worker or return 404)
      if (url.pathname.startsWith('/recover') || url.pathname.startsWith('/api/recover'))
        return new Response('Use core Worker for recovery', { status: 404 });
      return new Response('vm-mod-worker: ' + url.pathname, { status: 404 });
    } catch (e) {
      return new Response(JSON.stringify({ error: e.message }), { status: 500,
        headers: { 'Content-Type': 'application/json' } });
    }
  }
};

function json(data, status = 200) {
  return new Response(JSON.stringify(data), { status,
    headers: { 'Content-Type': 'application/json' } });
}
