# -*- coding: utf-8 -*-
"""本地假 Steam 商店：用来离线跑通「搜索 -> 匹配 -> 下载封面」整条链路。

用法: python scripts/mock_steam.py 8899
"""
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import urlparse, parse_qs

# 假封面：一张足够大的 PNG（> 2000 字节即可通过校验）
TINY_PNG = bytes.fromhex(
    "89504e470d0a1a0a0000000d4948445200000010000000100806000000"
    "1ff3ff61000000"
) + b"\x00" * 4000 + bytes.fromhex("49454e44ae426082")


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):  # 安静一点
        sys.stderr.write("[mock] " + (fmt % args) + "\n")

    def do_GET(self):
        u = urlparse(self.path)
        q = parse_qs(u.query)
        term = (q.get("term") or [""])[0]
        lang = (q.get("l") or [""])[0]
        cc = (q.get("cc") or [""])[0]

        if u.path == "/api/storesearch/":
            # 只有 schinese + cc=us 才返回结果，模拟真实情况下的分区差异
            if lang == "schinese" and cc == "us":
                items = [
                    {"type": "app", "id": 4405120, "name": "影之刃零"},
                    {"type": "app", "id": 9999999, "name": "影之刃零数字豪华版升级包"},
                ]
            else:
                items = []
            body = json.dumps({"total": len(items), "items": items}, ensure_ascii=False).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return

        if u.path.startswith("/steam/apps/") and u.path.endswith("/header.jpg"):
            self.send_response(200)
            self.send_header("Content-Type", "image/png")
            self.send_header("Content-Length", str(len(TINY_PNG)))
            self.end_headers()
            self.wfile.write(TINY_PNG)
            return

        self.send_response(404)
        self.end_headers()


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8899
    HTTPServer(("127.0.0.1", port), Handler).serve_forever()
