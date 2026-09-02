"""客户端真实 IP 解析"""
from fastapi import Request

from app.config import settings


def get_client_ip(request: Request) -> str | None:
    """获取客户端真实 IP。

    每个可信代理会把它看到的来源地址追加到 X-Forwarded-For 末尾，
    而直接连接本应用的那一层代理就是 TCP 对端，不在头里。因此声明了
    N 层可信代理（TRUSTED_PROXY_COUNT = N > 0）时，真实客户端是倒数
    第 N 个条目；这一定义与 Express 的 trust proxy / nginx real_ip
    模块一致。N = 0 时忽略转发头，直接取 TCP 对端地址，避免伪造。
    """
    count = settings.trusted_proxy_count
    if count > 0:
        forwarded_for = request.headers.get("x-forwarded-for")
        if forwarded_for:
            entries = [e.strip() for e in forwarded_for.split(",") if e.strip()]
            if entries:
                if len(entries) >= count:
                    return entries[-count]
                return entries[0]
    return request.client.host if request.client else None
