// Cloudflare Worker — policy-gateway 公网节点
// 与路由器 Rust 后端共享同一套 API，权限表存 KV。
//
// 功能:
//   /api/signup             证书申请
//   /api/signup/status      状态查询
//   /manager                审批面板
//   /api/manager/approve    审批操作
//   /recover                根证书恢复页面
//   /api/recover/setup-pin  生成恢复 PIN (需 admin)
//   /api/recover/verify     验证 PIN + 签新根证书
//   /sync/pull              路由器拉取 Worker 数据
//   /sync/push              路由器推送数据到 Worker

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (request.method === 'OPTIONS') {
      return new Response(null, {
        headers: { 'Access-Control-Allow-Origin': '*', 'Access-Control-Allow-Methods': 'POST,GET,OPTIONS' }
      });
    }

    try {
      // --- Existing API ---
      if (request.method === 'POST' && url.pathname === '/api/signup') return handleSignup(request, env);
      if (request.method === 'GET' && url.pathname === '/api/signup/status') return handleStatus(request, url, env);
      if (request.method === 'GET' && url.pathname === '/manager') return handleManager(request, env);
      if (request.method === 'POST' && url.pathname === '/api/manager/approve') return handleApprove(request, env);
      if (request.method === 'GET' && url.pathname === '/sync/pull') return handleSyncPull(env);
      if (request.method === 'POST' && url.pathname === '/sync/push') return handleSyncPush(request, env);

      // --- Recovery ---
      if (request.method === 'GET' && url.pathname === '/recover') return handleRecoverPage(env);
      if (request.method === 'POST' && url.pathname === '/api/recover/setup-pin') return handleSetupPin(request, env);
      if (request.method === 'POST' && url.pathname === '/api/recover/verify') return handleRecoverVerifyToken(request, env);

      // --- Static assets ---
      if (url.pathname === '/' || url.pathname === '/index.html') return handleIndex(env);

      return new Response('policy-gateway Worker\n', { status: 404 });
    } catch (e) {
      return new Response(JSON.stringify({ error: e.message }), {
        status: 500, headers: { 'Content-Type': 'application/json' }
      });
    }
  }
};

// ============================================================
//  Recovery — 根证书恢复
// ============================================================

/// GET /recover — 恢复引导页面
async function handleRecoverPage(env) {
  const html = `<!DOCTYPE html>
<html lang="zh">
<head><meta charset="UTF-8"><title>证书恢复 — policy-gateway</title>
<style>
body{font-family:sans-serif;max-width:600px;margin:auto;padding:20px;line-height:1.6}
input{width:100%;padding:8px;margin:6px 0;font-size:16px;box-sizing:border-box}
button{width:100%;padding:10px;margin:6px 0;font-size:16px;cursor:pointer}
.msg{padding:10px;margin:6px 0;border-radius:4px}
.ok{background:#d4edda;color:#155724}
.err{background:#f8d7da;color:#721c24}
.info{background:#d1ecf1;color:#0c5460}
pre{background:#f4f4f4;padding:10px;overflow-x:auto;font-size:13px}
</style>
</head>
<body>
<h1>🔐 证书恢复</h1>
<p class="info">根证书丢失？如果你设置了恢复 PIN，可以在这里重新签发。</p>

<div id="step1">
  <h3>输入恢复 Token</h3>
  <input type="text" id="token" placeholder="恢复 Token" autocomplete="off">
  <button onclick="verifyPin()">验证 PIN 并签发新证书</button>
  <div id="msg1"></div>
</div>

<div id="step2" style="display:none">
  <h3>新根证书已签发</h3>
  <p>请下载并安装到你的设备：</p>
  <pre id="certOutput"></pre>
  <button onclick="downloadCert()">下载证书 (.pem)</button>
  <p class="info">安装后即可访问 /manager 审批其他设备。</p>
</div>

<script>
async function verifyPin() {
  const pin = document.getElementById('pin').value;
  const msg = document.getElementById('msg1');
  msg.innerHTML = '⏳ 验证中...';
  msg.className = 'msg';

  try {
    const r = await fetch('/api/recover/verify', {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({ token })
    });
    const d = await r.json();
    if (r.ok) {
      document.getElementById('step1').style.display = 'none';
      document.getElementById('step2').style.display = 'block';
      document.getElementById('certOutput').textContent = d.cert;
      window._recoverCert = d.cert;
      msg.className = 'msg ok';
      msg.textContent = d.message;
    } else {
      msg.className = 'msg err';
      msg.textContent = d.error || '验证失败';
    }
  } catch(e) {
    msg.className = 'msg err';
    msg.textContent = '请求失败: ' + e.message;
  }
}

function downloadCert() {
  if (!window._recoverCert) return;
  const blob = new Blob([window._recoverCert], {type: 'application/x-pem-file'});
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = 'policy-gateway-root-recovered.pem';
  a.click();
}
</script>
<p style="margin-top:20px;font-size:13px;color:#666">
还没有恢复 Token？联系路由器管理员存在云盘/密码管理器中的长期凭证。
</p>
</body>
</html>`;
  return new Response(html, { headers: { 'Content-Type': 'text/html; charset=utf-8' } });
}

/// POST /api/recover/setup-pin — 管理员生成恢复 PIN
/// 需要有效的 admin token 认证
async function handleSetupPin(request, env) {
  const body = await request.json();
  const token = body.token;
  if (!token || token !== env.MANAGER_TOKEN) {
    return json({ error: 'token 无效' }, 401);
  }

  // 生成 6 位数字 PIN
  const pin = Math.floor(100000 + Math.random() * 900000).toString();
  const expiresAt = Date.now() + 7 * 24 * 3600 * 1000; // 7 天有效

  await env.RECOVERY_PINS.put(pin, JSON.stringify({
    pin, created_at: Date.now(), expires_at: expiresAt, used: false
  }));

  return json({
    status: 'ok',
    pin,
    expires_at: new Date(expiresAt).toISOString(),
    message: '恢复 PIN 已生成，7 天内有效。请妥善保存！'
  });
}

/// POST /api/recover/verify — 验证 PIN + 签发新根证书
async function handleRecoverVerifyToken(request, env) {
  const { token } = await request.json();
  if (!token || token.length < 16) {
    return json({ error: '\u8bf7\u8f93\u5165\u6062\u590d Token' }, 400);
  }
  const expected = env.RECOVERY_TOKEN;
  if (!expected) {
    return json({ error: '\u6062\u590d\u529f\u80fd\u672a\u914d\u7f6e' }, 501);
  }
  if (token.length !== expected.length) {
    return json({ error: 'Token \u65e0\u6548' }, 401);
  }
  let match = 0;
  for (let i = 0; i < token.length; i++) {
    match |= token.charCodeAt(i) ^ expected.charCodeAt(i);
  }
  if (match !== 0) {
    return json({ error: 'Token \u65e0\u6548' }, 401);
  }
  const cert = await generateSelfSignedCert('policy-gateway-recovered-root');
  const sha256 = await certSha256(cert);
  const entry = JSON.stringify({
    sha256, hostname: '\u6062\u590d\u7684\u6839\u8bc1\u4e66', bitmap: 0xFF,
    status: 'active', created_at: Date.now(), recovered: true
  });
  await env.AUTH_TABLE.put(sha256, entry);
  return json({ status: 'ok', message: 'ok', cert });
}

// 生成新的根证书（在 Worker 中用 Web Crypto API）
  // 自签名证书，标记为根管理员
  const cert = await generateSelfSignedCert('policy-gateway-recovered-root');
  const sha256 = await certSha256(cert);

  // 写入权限表（全权限）
  const entry = JSON.stringify({
    sha256, hostname: '恢复的根证书', bitmap: 0xFF,
    status: 'active', created_at: Date.now(), recovered: true
  });
  await env.AUTH_TABLE.put(sha256, entry);
  await env.PENDING_QUEUE.put(`recovered-${sha256.slice(0,8)}`, sha256);

  return json({
    status: 'ok',
    message: '新根证书已签发，同步到路由器后即可使用',
    cert
  });
}

/// 使用 Worker 原生 Web Crypto API 生成自签名证书
async function generateSelfSignedCert(cn) {
  // 生成 ECDSA P-256 密钥对
  const keyPair = await crypto.subtle.generateKey({
    name: 'ECDSA', namedCurve: 'P-256'
  }, true, ['sign']);

  // 导出公钥
  const pubKeyRaw = await crypto.subtle.exportKey('raw', keyPair.publicKey);
  const pubKeyB64 = btoa(String.fromCharCode(...new Uint8Array(pubKeyRaw)));

  // 构造自签名证书（简单格式）
  const serial = Date.now().toString(16);
  const notBefore = Math.floor(Date.now() / 1000) - 3600;
  const notAfter = notBefore + 3650 * 86400;

  // 简化的 PEM 格式——实际部署建议用更完整的 ASN.1 编码
  // 这里使用伪证书结构，路由器端验证时实际只看 SHA256
  const pem = `-----BEGIN CERTIFICATE-----
recovery:${cn}
serial:${serial}
pubkey:${pubKeyB64}
issued:${notBefore}
expires:${notAfter}
role:root
-----END CERTIFICATE-----`;

  return pem;
}

async function certSha256(certPem) {
  const enc = new TextEncoder();
  const data = enc.encode(certPem);
  const hash = await crypto.subtle.digest('SHA-256', data);
  return bytesToHex(new Uint8Array(hash));
}

// ============================================================
//  Existing API Handlers (unchanged from original)
// ============================================================

async function handleSignup(request, env) {
  const { cert, hostname } = await request.json();
  if (!cert || !hostname) return json({ error: 'cert 和 hostname 必填' }, 400);
  const encoder = new TextEncoder();
  const certBytes = encoder.encode(cert);
  const hashBuffer = await crypto.subtle.digest('SHA-256', certBytes);
  const sha256 = bytesToHex(new Uint8Array(hashBuffer));
  const existing = await env.AUTH_TABLE.get(sha256);
  if (existing) return json({ error: '证书已存在' }, 409);
  const requestId = crypto.randomUUID();
  const entry = JSON.stringify({ sha256, hostname, bitmap: 0, status: 'pending', requestId, createdAt: Date.now() });
  await env.AUTH_TABLE.put(sha256, entry);
  await env.PENDING_QUEUE.put(requestId, sha256);
  return json({ request_id: requestId, status: 'pending', sha256 });
}

async function handleStatus(request, url, env) {
  const id = url.searchParams.get('id');
  const sha256 = url.searchParams.get('sha256');
  let key = sha256;
  if (!key && id) key = await env.PENDING_QUEUE.get(id);
  if (!key) return json({ status: 'not_found' });
  const raw = await env.AUTH_TABLE.get(key);
  if (!raw) return json({ status: 'not_found' });
  const entry = JSON.parse(raw);
  return json({ status: entry.status, hostname: entry.hostname, bitmap: entry.bitmap ? BigInt(entry.bitmap).toString(16) : null });
}

async function handleManager(request, env) {
  let rows = '';
  const list = await env.AUTH_TABLE.list();
  for (const key of list.keys) {
    const raw = await env.AUTH_TABLE.get(key.name);
    if (!raw) continue;
    const e = JSON.parse(raw);
    if (e.status !== 'pending') continue;
    rows += `<tr><td>${e.hostname}</td><td><code>${e.sha256.slice(0,16)}...</code></td><td>${e.status}</td>`;
    rows += `<td><form action="/api/manager/approve" method="post" style="display:inline">`;
    rows += `<input type="hidden" name="requestId" value="${e.requestId}" />`;
    rows += `<input type="hidden" name="action" value="approve" />`;
    rows += `<label>bitmap: <input name="bitmap" value="01" size="4" /></label>`;
    rows += `<button type="submit">✅ 同意</button></form></td></tr>`;
  }
  const html = `<!DOCTYPE html><html lang="zh"><head><meta charset="UTF-8"><title>Worker 审批面板</title>`;
  return new Response(html + getManagerHtml(rows), { headers: { 'Content-Type': 'text/html; charset=utf-8' } });
}

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

async function handleSyncPull(env) {
  const entries = [];
  const list = await env.AUTH_TABLE.list();
  for (const key of list.keys) {
    const raw = await env.AUTH_TABLE.get(key.name);
    if (raw) entries.push(JSON.parse(raw));
  }
  // 也同步 recovery PIN 状态
  const pins = [];
  const pinList = await env.RECOVERY_PINS.list();
  for (const key of pinList.keys) {
    const raw = await env.RECOVERY_PINS.get(key.name);
    if (raw) pins.push(JSON.parse(raw));
  }
}

async function handleSyncPush(request, env) {
  const packet = await request.json();
  for (const entry of packet.entries) {
    await env.AUTH_TABLE.put(entry.sha256, JSON.stringify(entry));
  }
  return json({ status: 'synced', count: packet.entries.length });
}

function handleIndex(env) {
  const html = `<!DOCTYPE html><html lang="zh"><head><meta charset="UTF-8"><title>policy-gateway</title></head><body>
<h1>🔐 policy-gateway — Cloudflare Worker</h1>
<p>公网节点运行正常。</p>
<p><a href="/signup">申请证书</a> | <a href="/manager">管理面板</a> | <a href="/recover">证书恢复</a></p>
</body></html>`;
  return new Response(html, { headers: { 'Content-Type': 'text/html; charset=utf-8' } });
}

function getManagerHtml(rows) {
  return `<style>body{font-family:sans-serif;max-width:800px;margin:auto;padding:20px}
table{width:100%;border-collapse:collapse}td,th{border:1px solid #ddd;padding:8px}button{cursor:pointer}</style>
<body><h1>☁️ Worker 审批面板</h1><table><tr><th>主机名</th><th>SHA256</th><th>状态</th><th>操作</th></tr>${rows}</table>
<p><a href="/recover">证书恢复</a> | <small>bitmap: 01=connector</small></p></body></html>`;
}

function json(data, status = 200) {
  return new Response(JSON.stringify(data), { status, headers: { 'Content-Type': 'application/json' } });
}

function bytesToHex(bytes) {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
}
