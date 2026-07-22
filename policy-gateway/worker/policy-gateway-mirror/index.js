// policy-gateway-mirror — Cloudflare Pages 镜像模块
// 由 vm-mod-worker 管理版本。提供与路由器一致的 /signup + /manager + /permissions。
//
// 部署: 通过 vm-mod-worker install policy-gateway-mirror
// 版本: 由 vm-mod-worker 管理，支持回滚

// === 静态 HTML 页面 ===
const SIGNUP_HTML = `<!DOCTYPE html><html lang="zh"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>证书申请</title><style>
body{font-family:sans-serif;max-width:600px;margin:auto;padding:20px}
input,select,textarea{width:100%;padding:8px;margin:6px 0;box-sizing:border-box}
button{padding:10px 20px;background:#06c;color:#fff;border:none;cursor:pointer}
</style></head><body>
<h1>📜 证书申请</h1>
<p style="background:#f0f8ff;padding:12px;border-radius:8px;font-size:14px">
🤖 MCU: <code>curl -X POST ... -d '{"pubkey":"<hex>","hostname":"dev"}'</code></p>
<button onclick="genKey()">🔑 生成本地密钥对</button><span id="ks"></span>
<form id="f" style="display:none">
<input type="text" id="h" placeholder="设备名" required>
<select id="t"><option value="01">🌐 上网</option><option value="05">⚙️ 上网+计算</option></select>
<button type="submit" onclick="sub(event)">提交</button></form><div id="r"></div>
<script>
let kp;async function genKey(){try{
kp=await crypto.subtle.generateKey({name:'RSA-PSS',modulusLength:2048,publicExponent:new Uint8Array([1,0,1]),hash:'SHA-256'},false,['sign']);
document.getElementById('ks').innerHTML='✅';document.getElementById('f').style.display='block'
}catch(e){alert('浏览器不支持 Web Crypto')}}
async function sub(e){e.preventDefault();const h=document.getElementById('h').value;
const spki=await crypto.subtle.exportKey('spki',kp.publicKey);
const b64=btoa(String.fromCharCode(...new Uint8Array(spki)));
const pem='-----BEGIN PUBLIC KEY-----\n'+b64.match(/.{1,64}/g).join('\n')+'\n-----END PUBLIC KEY-----';
const r=await fetch('/api/signup',{method:'POST',headers:{'Content-Type':'application/json'},
body:JSON.stringify({cert:pem,hostname:h,requested:document.getElementById('t').value})});
const d=await r.json();
document.getElementById('r').innerHTML='<p>✅ ID: <code>'+d.request_id+'</code></p>'+(d.cert_pem?'<details><summary>📄</summary><pre>'+d.cert_pem+'</pre></details>':'');
}</script>
<p><a href="/manager">管理</a> <a href="/permissions">权限表</a></p>
</body></html>`;

const MANAGER_HTML = `<!DOCTYPE html><html lang="zh"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>管理面板</title><style>
body{font-family:sans-serif;max-width:800px;margin:auto;padding:20px}
input{width:100%;padding:8px}button{padding:10px;background:#06c;color:#fff;border:none;cursor:pointer}
table{width:100%;border-collapse:collapse}td,th{border:1px solid #ddd;padding:8px}
</style></head><body>
<h1>🔑 管理面板</h1>
<input type="text" id="t" placeholder="Token">
<button onclick="load()">解锁</button>
<div id="list"></div>
<script>
async function load(){const t=document.getElementById('t').value;if(!t)return;
const r=await(await fetch('/api/manager/pending?token='+t)).json();
let h='<h3>待审批: '+r.count+'</h3><table><tr><th>设备</th><th>状态</th><th>操作</th></tr>';
r.entries.forEach(e=>{h+='<tr><td>'+e.hostname+'</td><td>'+e.status+'</td>'+
'<td><button onclick="appr(\''+e.request_id+'\')">✅</button>'+
'<button onclick="rej(\''+e.request_id+'\')">❌</button></td></tr>'});
document.getElementById('list').innerHTML=h+'</table>';
}
async function appr(id){const t=document.getElementById('t').value;
await fetch('/api/manager/approve',{method:'POST',headers:{'Content-Type':'application/json'},
body:JSON.stringify({request_id:id,action:'approve',bitmap:1,token:t})});load();}
async function rej(id){const t=document.getElementById('t').value;
await fetch('/api/manager/approve',{method:'POST',headers:{'Content-Type':'application/json'},
body:JSON.stringify({request_id:id,action:'reject',token:t})});load();}
</script>
<p><a href="/signup">申请</a> <a href="/permissions">权限表</a></p>
</body></html>`;

// === 处理函数 ===
async function handleSignup(request, env) {
  const url = new URL(request.url);
  if (request.method === 'GET') {
    return new Response(SIGNUP_HTML, { headers: { 'Content-Type': 'text/html;charset=utf-8' } });
  }
  const { csr, cert, pubkey, hostname, requested } = await request.json();
  if (!hostname) return json({ error: 'hostname required' }, 400);
  const id = crypto.randomUUID();
  const entry = { request_id: id, hostname, csr: csr || null, cert: cert || null,
    pubkey: pubkey || null, requested: requested || '01',
    status: 'pending_router_sign', created_at: Date.now() };
  await env.PENDING_QUEUE.put(id, JSON.stringify(entry));
  return json({ request_id: id, status: 'pending_router_sign',
    sha256: 'pending_' + id.slice(0, 8) });
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
  const mt = env.MANAGER_TOKEN || '';
  if (token !== mt) return json({ error: 'unauthorized' }, 401);
  if (request.method === 'GET') {
    if (url.pathname === '/manager') {
      return new Response(MANAGER_HTML, { headers: { 'Content-Type': 'text/html;charset=utf-8' } });
    }
    const pending = []; const list = await env.PENDING_QUEUE.list();
    for (const k of list.keys) { const r = await env.PENDING_QUEUE.get(k.name); if (r) pending.push(JSON.parse(r)); }
    return json({ count: pending.length, entries: pending });
  }
  const { request_id, action } = await request.json();
  if (action === 'approve') {
    const raw = await env.PENDING_QUEUE.get(request_id);
    if (!raw) return json({ error: 'not found' }, 404);
    const e = JSON.parse(raw); e.status = 'approved';
    await env.PENDING_QUEUE.put(request_id, JSON.stringify(e));
    return json({ status: 'approved' });
  }
  if (action === 'reject') {
    await env.PENDING_QUEUE.delete(request_id);
    return json({ status: 'rejected' });
  }
  return json({ error: 'unknown' }, 400);
}

async function handlePermissions(env) {
  const entries = []; const list = await env.AUTH_TABLE.list();
  for (const k of list.keys) {
    const r = await env.AUTH_TABLE.get(k.name);
    if (r) { try { entries.push({ sha256: k.name, ...JSON.parse(r) }); } catch(e) {} }
  }
  let html = '<!DOCTYPE html><html lang="zh"><head><meta charset="UTF-8"><title>权限表</title>'
    + '<style>body{font-family:sans-serif;max-width:800px;margin:auto;padding:20px}'
    + 'table{width:100%;border-collapse:collapse}td,th{border:1px solid #ddd;padding:8px}</style></head><body>'
    + '<h1>📋 权限表 (' + entries.length + ')</h1><table><tr><th>主机名</th><th>SHA256</th><th>位图</th><th>状态</th></tr>';
  for (const e of entries) {
    html += '<tr><td>' + (e.hostname || '—') + '</td><td><code>' + e.sha256.slice(0, 12) + '…</code></td>'
      + '<td>' + (e.bitmap || 0) + '</td><td>' + (e.status || '—') + '</td></tr>';
  }
  html += '</table><p><a href="/manager">管理</a> <a href="/signup">申请</a></p></body></html>';
  return new Response(html, { headers: { 'Content-Type': 'text/html;charset=utf-8' } });
}

async function handleHelp() {
  const data = [
    { topic: 'mcu', title: 'MCU 接入', steps: [
      { title: 'pubkey 直发', body: 'POST /api/signup with pubkey field' },
      { title: 'CSR 提交', body: 'POST /api/signup with csr field' },
    ]},
    { topic: 'browser', title: '浏览器', steps: [
      { title: '访问 /signup', body: '生成本地密钥对 → 提交' },
    ]},
  ];
  return json(data);
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
      if (url.pathname.startsWith('/signup') && url.pathname !== '/signup/status')
        return handleSignup(request, env);
      if (url.pathname === '/signup/status' || url.pathname === '/api/signup/status')
        return handleStatus(request, env);
      if (url.pathname === '/manager' || url.pathname === '/api/manager/approve'
          || url.pathname === '/api/manager/pending')
        return handleManager(request, env);
      if (url.pathname === '/permissions') return handlePermissions(env);
      if (url.pathname === '/api/help') return handleHelp();
      if (url.pathname.startsWith('/recover'))
        return new Response('Use core Worker for recovery', { status: 404 });
      return new Response('policy-gateway-mirror v0.1\n', { status: 404 });
    } catch (e) {
      return json({ error: e.message }, 500);
    }
  }
};

function json(data, status = 200) {
  return new Response(JSON.stringify(data), { status, headers: { 'Content-Type': 'application/json' } });
}
