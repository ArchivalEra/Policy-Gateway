// Cloudflare Worker — policy-gateway 公网恢复节点
// 职责单一: 仅处理根证书恢复（通过预置 RECOVERY_TOKEN）。
// 其他所有功能（signup/manager/审批/同步）都是可选模块，不在此 Worker 中。

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (request.method === 'OPTIONS') {
      return new Response(null, {
        headers: { 'Access-Control-Allow-Origin': '*', 'Access-Control-Allow-Methods': 'POST,GET,OPTIONS' }
      });
    }

    try {
      // 只有 /recover 和 /api/recover/* 路径
      if (request.method === 'GET' && url.pathname === '/recover') return handleRecoverPage(env);
      if (request.method === 'POST' && url.pathname === '/api/recover/verify') return handleRecoverVerify(request, env);

      // 根路径：简单状态页
      if (url.pathname === '/' || url.pathname === '/index.html') {
        return new Response(`policy-gateway Worker (recovery only)\n`, { headers: { 'Content-Type': 'text/plain' } });
      }

      return new Response('404 — 此 Worker 仅处理 /recover，其他功能请访问路由器\n', { status: 404 });
    } catch (e) {
      return new Response(JSON.stringify({ error: e.message }), {
        status: 500, headers: { 'Content-Type': 'application/json' }
      });
    }
  }
};

// ============================================================
//  Recovery — 根证书恢复（唯一功能）
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
</style></head>
<body>
<h1>🔐 证书恢复</h1>
<p class="info">根证书丢失后，用存储在云盘/密码管理器中的恢复 Token 重新签发。</p>
<div id="step1">
  <h3>输入恢复 Token</h3>
  <input type="password" id="token" placeholder="恢复 Token" autocomplete="off">
  <button onclick="doRecover()">验证并签发新根证书</button>
  <div id="msg1" class="msg"></div>
</div>
<div id="step2" style="display:none">
  <h3>✅ 新根证书已签发</h3>
  <p class="ok">请下载并安装：</p>
  <pre id="certOutput" style="background:#f4f4f4;padding:10px;"></pre>
  <button onclick="downloadCert()">⬇ 下载证书 (.pem)</button>
  <p class="info">安装证书后，用新证书访问路由器 /manager 审批设备。</p>
</div>
<script>
async function doRecover() {
  const token = document.getElementById('token').value;
  const msg = document.getElementById('msg1');
  msg.textContent = '⏳ 验证中...'; msg.className = 'msg';
  try {
    const r = await fetch('/api/recover/verify', {
      method: 'POST', headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({ token })
    });
    const d = await r.json();
    if (r.ok) {
      document.getElementById('step1').style.display = 'none';
      document.getElementById('step2').style.display = 'block';
      document.getElementById('certOutput').textContent = d.cert;
      window._rc = d.cert;
      msg.className = 'msg ok'; msg.textContent = '✅ ' + d.message;
    } else {
      msg.className = 'msg err'; msg.textContent = '❌ ' + (d.error || '验证失败');
    }
  } catch(e) {
    msg.className = 'msg err'; msg.textContent = '❌ 请求失败: ' + e.message;
  }
}
function downloadCert() {
  if (!window._rc) return;
  const b = new Blob([window._rc], {type: 'application/x-pem-file'});
  const a = document.createElement('a');
  a.href = URL.createObjectURL(b);
  a.download = 'policy-gateway-root.pem';
  a.click();
}
</script>
<p style="margin-top:20px;font-size:13px;color:#666">
恢复 Token 在首次设置时由管理员生成，存在云盘/密码管理器中。
</p>
</body>
</html>`;
  return new Response(html, { headers: { 'Content-Type': 'text/html; charset=utf-8' } });
}

/// POST /api/recover/verify — 验证 Token + 签发新根证书
async function handleRecoverVerify(request, env) {
  const { token } = await request.json();
  if (!token || token.length < 16) {
    return json({ error: '请输入有效的恢复 Token' }, 400);
  }

  // 恒定时间比较
  const expected = env.RECOVERY_TOKEN;
  if (!expected) return json({ error: '恢复功能未配置（未设置 RECOVERY_TOKEN）' }, 501);
  if (token.length !== expected.length) return json({ error: 'Token 无效' }, 401);
  let match = 0;
  for (let i = 0; i < token.length; i++) match |= token.charCodeAt(i) ^ expected.charCodeAt(i);
  if (match !== 0) return json({ error: 'Token 无效' }, 401);

  // 签发新根证书
  const cert = await generateSelfSignedCert('policy-gateway-recovered-root');
  const sha256 = await certSha256(cert);

  // 存入权限表（全权限）
  const entry = JSON.stringify({
    sha256, hostname: 'recovered-root', bitmap: 'FF',
    status: 'active', created_at: Date.now(), recovered: true
  });
  await env.AUTH_TABLE.put(sha256, entry);

  return json({ status: 'ok', message: '新根证书已签发，同步到路由器后即可使用', cert });
}

// ============================================================
//  Crypto helpers
// ============================================================

async function generateSelfSignedCert(cn) {
  const keyPair = await crypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign']);
  const pubKeyRaw = await crypto.subtle.exportKey('raw', keyPair.publicKey);
  const pubKeyB64 = btoa(String.fromCharCode(...new Uint8Array(pubKeyRaw)));
  const serial = Date.now().toString(16);
  const notBefore = Math.floor(Date.now() / 1000) - 3600;
  const notAfter = notBefore + 3650 * 86400;
  return `-----BEGIN CERTIFICATE-----
recovery:${cn}
serial:${serial}
pubkey:${pubKeyB64}
issued:${notBefore}
expires:${notAfter}
role:root
-----END CERTIFICATE-----`;
}

async function certSha256(certPem) {
  const enc = new TextEncoder();
  const hash = await crypto.subtle.digest('SHA-256', enc.encode(certPem));
  return bytesToHex(new Uint8Array(hash));
}

function bytesToHex(bytes) {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
}

function json(data, status = 200) {
  return new Response(JSON.stringify(data), { status, headers: { 'Content-Type': 'application/json' } });
}
