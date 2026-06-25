import base64
import hashlib
import hmac
import json
import secrets
import time

from fastapi import Header, HTTPException

from app.config import settings

ADMIN_SESSION_TTL_SECONDS = 24 * 60 * 60


def _admin_secret() -> str:
    if not settings.admin_password:
        raise HTTPException(status_code=503, detail="管理员密码未配置")
    return settings.admin_password


def verify_admin_password(password: str) -> None:
    if not hmac.compare_digest(password, _admin_secret()):
        raise HTTPException(status_code=401, detail="密码错误")


def _base64url_encode(value: bytes) -> str:
    return base64.urlsafe_b64encode(value).decode("ascii").rstrip("=")


def _base64url_decode(value: str) -> bytes:
    return base64.urlsafe_b64decode(value + "=" * (-len(value) % 4))


def _sign(payload: str) -> str:
    signature = hmac.new(
        _admin_secret().encode("utf-8"),
        payload.encode("ascii"),
        hashlib.sha256,
    ).digest()
    return _base64url_encode(signature)


def create_admin_token() -> tuple[str, int]:
    expires_at = int(time.time()) + ADMIN_SESSION_TTL_SECONDS
    payload = _base64url_encode(
        json.dumps(
            {"exp": expires_at, "nonce": secrets.token_urlsafe(16)},
            separators=(",", ":"),
        ).encode("utf-8")
    )
    return f"{payload}.{_sign(payload)}", expires_at


def verify_admin_token(authorization: str = Header("")) -> None:
    if not authorization.startswith("Bearer "):
        raise HTTPException(status_code=401, detail="请先登录管理员")

    token = authorization.removeprefix("Bearer ").strip()
    try:
        payload, signature = token.split(".", 1)
    except ValueError:
        raise HTTPException(status_code=401, detail="管理员登录无效")

    if not hmac.compare_digest(signature, _sign(payload)):
        raise HTTPException(status_code=401, detail="管理员登录无效")

    try:
        data = json.loads(_base64url_decode(payload))
        expires_at = int(data.get("exp", 0))
    except (ValueError, TypeError, json.JSONDecodeError):
        raise HTTPException(status_code=401, detail="管理员登录无效")

    if expires_at <= int(time.time()):
        raise HTTPException(status_code=401, detail="管理员登录已过期")
