#!/usr/bin/env python3
"""
图片分析工具 — Gemini REST API
模型: gemini-2.0-flash (快速、免费)
用法: describe_image <图片路径> [提示词]
依赖: Python 3 + Pillow (已有)
"""

import sys, json, base64, os, urllib.request, urllib.error, io
from pathlib import Path

API_KEY = os.environ.get("GEMINI_API_KEY", "")
MODEL = "gemini-2.0-flash"
API_URL = f"https://generativelanguage.googleapis.com/v1beta/models/{MODEL}:generateContent"

MIME_MAP = {
    ".jpg": "image/jpeg", ".jpeg": "image/jpeg",
    ".png": "image/png", ".gif": "image/gif",
    ".webp": "image/webp", ".bmp": "image/bmp",
}

def describe(path: str, prompt: str = "请详细描述这张图片里的内容，包括文字、物体、场景等。如果是终端或应用截图，请读出所有可见的文字。") -> str:
    if not API_KEY:
        return "❌ GEMINI_API_KEY 未设置"

    p = Path(path)
    if not p.exists():
        return f"❌ 文件不存在: {path}"

    mime = MIME_MAP.get(p.suffix.lower(), "image/jpeg")

    # Resize large images to avoid hitting limits
    try:
        from PIL import Image
        img = Image.open(path)
        w, h = img.size
        if max(w, h) > 1024:
            scale = 1024 / max(w, h)
            img = img.resize((int(w*scale), int(h*scale)), Image.LANCZOS)
        buf = io.BytesIO()
        img.save(buf, format='JPEG', quality=60)
        b64 = base64.b64encode(buf.getvalue()).decode()
    except Exception:
        with open(path, "rb") as f:
            b64 = base64.b64encode(f.read()).decode()

    payload = json.dumps({
        "contents": [{
            "parts": [
                {"text": prompt},
                {"inlineData": {"mimeType": mime, "data": b64}}
            ]
        }],
        "generationConfig": {"maxOutputTokens": 1024}
    }).encode()

    req = urllib.request.Request(
        f"{API_URL}?key={API_KEY}",
        data=payload,
        headers={"Content-Type": "application/json"},
        method="POST"
    )

    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            data = json.loads(resp.read())
            candidates = data.get("candidates", [])
            if not candidates:
                return f"空响应: {json.dumps(data, indent=2)[:300]}"
            parts = candidates[0].get("content", {}).get("parts", [])
            texts = [p.get("text", "") for p in parts if "text" in p]
            return "".join(texts) or f"无文本: {json.dumps(data,indent=2)[:300]}"
    except urllib.error.HTTPError as e:
        body = e.read().decode(errors='replace')[:500]
        return f"❌ Gemini HTTP {e.code}: {body}"
    except Exception as e:
        return f"❌ 错误: {e}"

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(f"用法: {sys.argv[0]} <图片路径> [提示词]")
        print(f"环境变量: GEMINI_API_KEY")
        sys.exit(1)
    prompt = sys.argv[2] if len(sys.argv) > 2 else "请详细描述这张图片里的内容，包括文字、物体、场景等。"
    result = describe(sys.argv[1], prompt)
    print(result)
