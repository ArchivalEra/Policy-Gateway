// ==PRE=RELEASE=v1.0//
// vm-worker — 最小恢复程序 + 快照管理
// 单 Worker 部署到 Cloudflare Pages
//
// 默认快照 (v1): 只暴露 /recover 恢复页面
// 未来通过快照切换加载 mirror 模块
//
// 路由:
//   /recover              — 恢复页面 (HTML)
//   /api/recover/setup    — 设置恢复码 (POST)
//   /api/recover/verify   — 验证恢复码 (POST)
//   /__version            — 当前快照版本
//   /__snapshot           — 列举快照 (GET) / 切换 (POST)

const SNAPSHOTS = {
  v1: { name: '恢复程序 v1', routes: ['/recover', '/api/recover/*'] },
};

const DEFAULT_SNAPSHOT = 'v1';
const RECOVERY_KV = 'RECOVERY';

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const path = url.pathname;
    const method = request.method;

    // --- 内部管理端点 ---
    if (path === '/__version') {
      const current = await env.CONFIG.get('current_snapshot') || DEFAULT_SNAPSHOT;
      return json({ snapshot: current, available: Object.keys(SNAPSHOTS) });
    }

    if (path === '/__snapshot') {
      if (method === 'POST') {
        const body = await request.json();
        const snap = body.snapshot;
        if (!SNAPSHOTS[snap]) {
          return json({ error: 'unknown snapshot' }, 400);
        }
        await env.CONFIG.put('current_snapshot', snap);
        return json({ status: 'switched', snapshot: snap });
      }
      return json({ current: await env.CONFIG.get('current_snapshot') || DEFAULT_SNAPSHOT, snapshots: SNAPSHOTS });
    }

    // --- 恢复端点 ---
    if (path === '/recover' && method === 'GET') {
      return new Response(RECOVER_HTML, {
        headers: { 'Content-Type': 'text/html; charset=utf-8' },
      });
    }

    if (path === '/api/recover/setup' && method === 'POST') {
      const { pin } = await request.json();
      if (!pin || pin.length < 8) {
        return json({ error: '恢复码至少 8 位' }, 400);
      }
      // 存 SHA256(PIN + env secret)
      const digest = await sha256(pin + (env.RECOVERY_SALT || 'vm-worker-default'));
      await env[RECOVERY_KV].put('recovery_hash', digest);
      return json({ status: 'ok', message: '恢复码已设置' });
    }

    if (path === '/api/recover/verify' && method === 'POST') {
      const { pin } = await request.json();
      const stored = await env[RECOVERY_KV].get('recovery_hash');
      if (!stored) {
        return json({ error: '未设置恢复码' }, 404);
      }
      const digest = await sha256(pin + (env.RECOVERY_SALT || 'vm-worker-default'));
      if (digest === stored) {
        return json({ status: 'ok', token: env.RECOVERY_TOKEN || '' });
      }
      return json({ error: '恢复码错误' }, 403);
    }

    // --- 同步端点 ---
    if (path === '/api/sync/push' && method === 'POST') {
      const body = await request.json();
      const auth = request.headers.get('X-Sync-Token') || body.token;
      if (auth !== env.WORKER_TOKEN) return json({ error: 'unauthorized' }, 401);
      
      // Store incoming events in KV
      const events = body.events || [];
      if (events.length > 0) {
        const key = `sync:router:${Date.now()}`;
        await env[RECOVERY_KV].put(key, JSON.stringify(events));
      }
      return json({ status: 'ok', received: events.length });
    }

    if (path === '/api/sync/pull' && method === 'GET') {
      const auth = url.searchParams.get('token');
      if (auth !== env.WORKER_TOKEN) return json({ error: 'unauthorized' }, 401);
      
      // Return pending events for the router
      const events = [];
      // TODO: list KV keys with prefix and return pending events
      return json({ status: 'ok', events });
    }

    // --- 404 ---
    return new Response('Not Found', { status: 404 });
  },
};

async function sha256(input) {
  const enc = new TextEncoder();
  const data = enc.encode(input);
  const hash = await crypto.subtle.digest('SHA-256', data);
  return Array.from(new Uint8Array(hash)).map(b => b.toString(16).padStart(2, '0')).join('');
}

function json(data, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: { 'Content-Type': 'application/json' },
  });
}

const RECOVER_HTML = `<!DOCTYPE html>
<html lang="zh">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>根证书恢复</title>
<style>
*{box-sizing:border-box}body{font-family:-apple-system,sans-serif;max-width:500px;margin:auto;padding:20px}
.card{background:#fff;border-radius:12px;padding:20px;margin:12px 0;box-shadow:0 1px 3px #0002}
input{width:100%;padding:10px;border:1px solid #ddd;border-radius:6px;font-size:16px;margin:6px 0}
button{width:100%;padding:12px;background:#06c;color:#fff;border:none;border-radius:6px;font-size:16px;cursor:pointer}
.success{color:#090}.error{color:#c00}
</style></head>
<body>
<div class="card"><h1>🔐 根证书恢复</h1>
<p>路由器根证书丢失后，通过此处重新获取管理权限。</p>
<div id="step-setup">
<h3>设置恢复码</h3>
<input type="password" id="pin-setup" placeholder="输入恢复码 (至少 8 位)" minlength="8">
<button onclick="setup()">设置</button>
</div>
<div id="step-verify" style="display:none">
<h3>验证恢复码</h3>
<input type="password" id="pin-verify" placeholder="输入恢复码">
<button onclick="verify()">验证</button>
</div>
<div id="result"></div>
</div>
<script>
async function setup(){
  const pin=document.getElementById('pin-setup').value;
  if(pin.length<8){document.getElementById('result').innerHTML='<p class="error">恢复码至少 8 位</p>';return}
  const r=await fetch('/api/recover/setup',{method:'POST',body:JSON.stringify({pin})});
  const d=await r.json();
  document.getElementById('result').innerHTML='<p class="success">'+d.message+'</p>';
  document.getElementById('step-setup').style.display='none';
  document.getElementById('step-verify').style.display='block';
}
async function verify(){
  const pin=document.getElementById('pin-verify').value;
  const r=await fetch('/api/recover/verify',{method:'POST',body:JSON.stringify({pin})});
  const d=await r.json();
  if(d.token){
    document.getElementById('result').innerHTML='<p class="success">✅ 验证通过，Token: <code>'+d.token+'</code></p>';
  }else{
    document.getElementById('result').innerHTML='<p class="error">'+d.error+'</p>';
  }
}
</script>
</body></html>`;
