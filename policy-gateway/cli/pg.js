// pg.js — policy-gateway CLI (零依赖, 任何 HTTP 环境)
// 与 Rust 版 pg 核心指令集完全一致。永不变。
// 用法:
//   node pg.js status
//   deno run pg.js approve <id>
//   PG_SERVER=x PG_TOKEN=y node pg.js pending

const S = process.env.PG_SERVER || 'http://localhost:8443';
const T = process.env.PG_TOKEN || '';
const H = process.env.PG_HOSTNAME || 'pg-cli';

async function api(m, p, b) {
  const r = await fetch(`${S}${p}`, {
    method: m,
    headers: b ? {'Content-Type':'application/json'} : {},
    body: b ? JSON.stringify(b) : void 0
  });
  return r.json();
}

async function main() {
  const c = process.argv[2], a1 = process.argv[3], a2 = process.argv[4];

  switch (c) {
    case 'status': case 'health':
      return console.log(JSON.stringify(await api('GET','/healthz')));

    case 'approve': case 'reject':
      if (!a1) return console.error('用法: pg', c, '<request_id>');
      return console.log(JSON.stringify(await api('POST','/api/manager/approve',
        {request_id:a1, action:c, bitmap:1, token:T})));

    case 'pending': case 'list':
      return console.log(JSON.stringify(await api('GET',`/api/manager/pending?token=${T}`),null,2));

    case 'cert':
      if (a1 === 'sign' && a2) {
        const isHex = /^[0-9a-f]{64,66}$/i.test(a2);
        const body = isHex
          ? {pubkey:a2, hostname:H, requested:'01'}
          : {csr:require('fs').readFileSync(a2,'utf8'), hostname:H, requested:'01'};
        return console.log(JSON.stringify(await api('POST','/api/signup',body),null,2));
      }
      if (a1 === 'status' && a2)
        return console.log(JSON.stringify(await api('GET',`/api/signup/status?sha256=${a2}`),null,2));
      return console.error('用法: pg cert sign <csr|pubkey> | status <sha256>');

    default:
      console.log('pg — CLI (核心指令集)\n');
      console.log('  pg status                   健康检查');
      console.log('  pg approve <id>             批准');
      console.log('  pg reject <id>              驳回');
      console.log('  pg pending                  待审批');
      console.log('  pg cert sign <csr|pubkey>   申请');
      console.log('  pg cert status <sha256>     状态\n');
      console.log('环境变量: PG_SERVER PG_TOKEN PG_HOSTNAME');
  }
}

main().catch(e => console.error('错误:', e.message));
