"""Small, local, heuristic visitor classification; no tracking SDK or network calls."""

import re
from urllib.parse import urlsplit

BOT = re.compile(
    r"bot|crawler|spider|slurp|preview|facebookexternalhit|headless|curl/|wget/|python-requests|httpx/|lighthouse|monitor|uptime",
    re.IGNORECASE,
)


def classify_agent(agent: str | None) -> dict:
    agent = agent or ""
    if not agent:
        device = "unknown"
    elif re.search(r"ipad|tablet|android(?!.*mobile)", agent, re.IGNORECASE):
        device = "tablet"
    elif re.search(r"mobile|iphone|ipod|android", agent, re.IGNORECASE):
        device = "mobile"
    elif re.search(r"windows|macintosh|x11|linux|cros", agent, re.IGNORECASE):
        device = "desktop"
    else:
        device = "unknown"
    browser = "unknown"
    for pattern, name in [
        (r"Edg(?:e|A|iOS)?/", "Edge"),
        (r"OPR/|Opera/", "Opera"),
        (r"Firefox/|FxiOS/", "Firefox"),
        (r"Chrome/|CriOS/", "Chrome"),
        (r"Version/.*Safari/", "Safari"),
    ]:
        if re.search(pattern, agent, re.IGNORECASE):
            browser = name
            break
    return {
        "device": device,
        "browser": browser,
        "is_bot": int(bool(BOT.search(agent))),
    }


def referrer_domain(value: str | None) -> str | None:
    try:
        parsed = urlsplit(value or "")
        if parsed.scheme not in {"http", "https"} or not parsed.hostname:
            return None
        domain = parsed.hostname.encode("idna").decode("ascii").lower()
        return domain[:253]
    except (ValueError, UnicodeError):
        return None
