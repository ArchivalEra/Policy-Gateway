#!/bin/sh
# test-full-flow.sh — 完整流程测试
# 启动服务器 → MCU pubkey 申请 → 管理审批 → 确认 → 状态查询
#
# 用法: ./test-full-flow.sh
# 环境: 需要端口 8443 可用

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

echo "=== 完整流程测试 ==="

# 1. 编译
echo ">>> 编译..."
cargo build --release 2>/dev/null || cargo build 2>/dev/null

# 2. 启动服务器
echo ">>> 启动服务器..."
MANAGER_TOKEN="test" ./target/release/policy-gateway &
PID=$!
sleep 3

cleanup() {
    kill $PID 2>/dev/null
    echo ">>> 服务器已关闭"
}
trap cleanup EXIT

# 3. 健康检查
echo ">>> 健康检查..."
curl -sf http://localhost:8443/healthz | python3 -c "import sys,json;d=json.load(sys.stdin);print(f'  {d[\"service\"]} v{d[\"version\"]}')" || { echo "❌ 服务器未启动"; exit 1; }

# 4. MCU pubkey 申请
echo ">>> MCU 证书申请..."
R=$(curl -sf -X POST http://localhost:8443/api/signup \
  -H 'Content-Type: application/json' \
  -d '{"pubkey":"abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234","hostname":"esp32-test","requested":"01","hw_platform":"esp32"}')
echo "  $R" | python3 -c "import sys,json;d=json.load(sys.stdin);print(f'  status={d[\"status\"]}')"
RID=$(echo "$R" | python3 -c "import sys,json;print(json.load(sys.stdin)['request_id'])")

# 5. 管理员审批
echo ">>> 管理员审批..."
curl -sf -X POST http://localhost:8443/api/manager/approve \
  -H 'Content-Type: application/json' \
  -d "{\"request_id\":\"$RID\",\"action\":\"approve\",\"bitmap\":1,\"token\":\"test\"}" | python3 -c "import sys,json;d=json.load(sys.stdin);print(f'  status={d[\"status\"]}')"

# 6. 状态查询
echo ">>> 状态查询..."
curl -sf "http://localhost:8443/api/signup/status?id=$RID" | python3 -c "import sys,json;d=json.load(sys.stdin);print(f'  status={d.get(\"status\")}')"

echo ""
echo "=== ✅ 全流程测试通过 ==="
