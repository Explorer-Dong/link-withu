from fastapi import APIRouter

from app.schemas import AdminLoginRequest, AdminLoginResponse
from app.services.admin_auth import create_admin_token, verify_admin_password

router = APIRouter(prefix="/api/admin", tags=["admin"])


@router.post("/login", response_model=AdminLoginResponse)
async def login(body: AdminLoginRequest):
    verify_admin_password(body.password)
    token, expires_at = create_admin_token()
    return AdminLoginResponse(token=token, expires_at=expires_at)
