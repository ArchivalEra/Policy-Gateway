// vm-mod-worker — Cloudflare Pages 模块版本管理器
// 功能: 安装/升级/回滚 Worker 端模块（如 policy-gateway-mirror）
//
// 类比: 与本地 policy-gateway-vm 相似，但管理的是 Pages 模块
//   vm (本地):  snapshot / rollback / install → policy-gateway 本体
//   vm-mod-worker (Pages): snapshot / rollback / install → mirror 等模块

// 模块注册表
const MODULES = {
  'policy-gateway-mirror': {
    description: 'Mirror router /manager + /signup on Cloudflare Pages',
    repo: 'https://github.com/ArchivalEra/Worker-Router-Gateway',
    entry: '/policy-gateway-mirror/index.js',
  },
};

// === CLI 风格的模块管理 ===

// 列举已安装模块
function listModules(env) {
  const installed = MODULES;
  return Object.entries(installed).map(([name, info]) => ({
    name,
    description: info.description,
    version: 'latest', // 从 KV 读取实际版本
    status: 'active',
  }));
}

// 安装/升级模块
async function installModule(name, version, env) {
  if (!MODULES[name]) return { error: 'unknown module: ' + name };
  // TODO: 从 GitHub/registry 下载模块版本
  // TODO: 写入 KV
  // TODO: 记录版本历史
  return { status: 'installed', module: name, version: version || 'latest' };
}

// 回滚模块到指定版本
async function rollbackModule(name, version, env) {
  // TODO: 从 KV 读取版本历史
  // TODO: 恢复到指定版本
  return { status: 'rolled_back', module: name, to: version };
}

// 创建当前模块状态快照
async function snapshotModule(name, env) {
  // TODO: 备份当前模块文件
  return { status: 'snapshot_created', module: name };
}

// === Router ===
export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (request.method === 'OPTIONS') {
      return new Response(null, {
        headers: { 'Access-Control-Allow-Origin': '*', 'Access-Control-Allow-Methods': 'GET,POST,OPTIONS' }
      });
    }
    try {
      const path = url.pathname;

      // 模块管理 API
      if (path === '/api/vm-mod/list') {
        return json(listModules(env));
      }
      if (path === '/api/vm-mod/install' && request.method === 'POST') {
        const { name, version } = await request.json();
        return json(await installModule(name, version, env));
      }
      if (path === '/api/vm-mod/rollback' && request.method === 'POST') {
        const { name, version } = await request.json();
        return json(await rollbackModule(name, version, env));
      }
      if (path === '/api/vm-mod/snapshot' && request.method === 'POST') {
        const { name } = await request.json();
        return json(await snapshotModule(name, env));
      }

      // 已安装模块的路由 — 交给对应模块处理
      // policy-gateway-mirror 模块处理 /signup /manager 等路径
      if (path.startsWith('/signup') || path.startsWith('/manager') ||
          path.startsWith('/permissions') || path.startsWith('/api/help')) {
        // 动态加载 mirror 模块
        const mirror = await import('./policy-gateway-mirror/index.js');
        return mirror.default.fetch(request, env);
      }

      return new Response('vm-mod-worker\n', { status: 404 });
    } catch (e) {
      return new Response(JSON.stringify({ error: e.message }), {
        status: 500, headers: { 'Content-Type': 'application/json' }
      });
    }
  }
};

function json(data, status = 200) {
  return new Response(JSON.stringify(data), { status, headers: { 'Content-Type': 'application/json' } });
}
