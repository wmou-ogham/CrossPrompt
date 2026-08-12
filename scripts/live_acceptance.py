#!/usr/bin/env python3
"""Exercise a running CrossPrompt stack without printing secrets or credentials."""

import argparse
import json
import urllib.error
import urllib.request


class Client:
    def __init__(self, base_url):
        self.base_url = base_url.rstrip("/")

    def request(self, method, path, payload=None, *, bearer=None, cookie=None, csrf=None, expected=(200,)):
        headers = {"Accept": "application/json"}
        body = None
        if payload is not None:
            headers["Content-Type"] = "application/json"
            body = json.dumps(payload).encode()
        if bearer:
            headers["Authorization"] = f"Bearer {bearer}"
        if cookie:
            headers["Cookie"] = cookie
        if csrf:
            headers["X-CSRF-Token"] = csrf
        request = urllib.request.Request(self.base_url + path, data=body, headers=headers, method=method)
        try:
            response = urllib.request.urlopen(request, timeout=10)
            status = response.status
            raw = response.read()
            response_headers = response.headers
        except urllib.error.HTTPError as error:
            status = error.code
            raw = error.read()
            response_headers = error.headers
        if status not in expected:
            safe_body = raw.decode(errors="replace")[:500]
            raise AssertionError(f"{method} {path} returned {status}: {safe_body}")
        value = json.loads(raw) if raw else None
        return status, value, response_headers


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", default="http://127.0.0.1:18080")
    parser.add_argument("--admin-password-file", required=True)
    args = parser.parse_args()
    password = open(args.admin_password_file, encoding="utf-8").read().strip()
    client = Client(args.base_url)

    _, created, _ = client.request(
        "POST", "/api/v1/vaults", {"name": "CrossPrompt live acceptance"}, expected=(201,)
    )
    secret = created["secret"]
    vault_id = created["vault"]["id"]

    _, artifact_types, _ = client.request("GET", "/api/v1/artifact-types")
    assert len(artifact_types) == 12
    skill_type = next(item for item in artifact_types if item["key"] == "skill")
    assert "執行流程" in skill_type["template"]

    _, block, _ = client.request(
        "POST", "/api/v1/blocks?source=live-acceptance",
        {"block_type": "skill", "title": "Research Skill", "content": "Verify primary sources."},
        bearer=secret, expected=(201,)
    )
    assert block["block_type"] == "skill"
    _, portable, _ = client.request(
        "POST", "/api/v1/portable-text", {"block_ids": [block["id"]]}, bearer=secret
    )
    assert "給接收 Agent 的使用說明" in portable["text"]
    assert "Skill / 專業技能 (`skill`)" in portable["text"]
    client.request(
        "POST", "/api/v1/bundles?source=live-acceptance",
        {"name": "Default", "block_ids": [block["id"]]}, bearer=secret, expected=(201,)
    )
    client.request(
        "PATCH", f"/api/v1/blocks/{block['id']}?source=live-acceptance",
        {"block_type": "skill", "title": block["title"], "content": "Temporary change", "version": block["version"]},
        bearer=secret,
    )
    _, revisions, _ = client.request("GET", "/api/v1/revisions", bearer=secret)
    update_revision = next(item for item in revisions if item["action"] == "update" and item["resource_type"] == "block")
    client.request("POST", f"/api/v1/revisions/{update_revision['id']}/restore", {}, bearer=secret)
    _, snapshot, _ = client.request("GET", "/api/v1/vault", bearer=secret)
    assert snapshot["blocks"][0]["content"] == "Verify primary sources."
    assert snapshot["blocks"][0]["block_type"] == "skill"
    assert "expires_at" not in snapshot["vault"]

    _, rotated, _ = client.request("POST", "/api/v1/vault/rotate-secret", {}, bearer=secret)
    client.request("GET", "/api/v1/vault", bearer=secret, expected=(401,))
    secret = rotated["secret"]

    _, login, login_headers = client.request(
        "POST", "/api/v1/admin/session", {"username": "admin", "password": password}
    )
    cookie = login_headers.get("Set-Cookie").split(";", 1)[0]
    csrf = login["csrf_token"]
    _, overview, _ = client.request("GET", "/api/v1/admin/overview", cookie=cookie)
    assert overview["vaults"]["total"] >= 1
    _, detail, _ = client.request("GET", f"/api/v1/admin/vaults/{vault_id}", cookie=cookie)
    assert detail["blocks"][0]["content"] == "Verify primary sources."
    assert detail["blocks"][0]["block_type"] == "skill"

    client.request(
        "POST", f"/api/v1/admin/vaults/{vault_id}/suspend", {"reason": "live acceptance"},
        cookie=cookie, csrf=csrf, expected=(204,)
    )
    client.request("GET", "/api/v1/vault", bearer=secret, expected=(423,))
    client.request(
        "POST", f"/api/v1/admin/vaults/{vault_id}/resume", {"reason": "live acceptance"},
        cookie=cookie, csrf=csrf, expected=(204,)
    )
    client.request("GET", "/api/v1/vault", bearer=secret)
    client.request(
        "POST", f"/api/v1/admin/vaults/{vault_id}/delete", {"reason": "live acceptance"},
        cookie=cookie, csrf=csrf, expected=(204,)
    )
    client.request("POST", "/api/v1/vault/restore", {}, bearer=secret, expected=(403,))
    client.request(
        "POST", f"/api/v1/admin/vaults/{vault_id}/restore", {"reason": "live acceptance"},
        cookie=cookie, csrf=csrf, expected=(204,)
    )
    client.request(
        "DELETE", f"/api/v1/admin/vaults/{vault_id}/permanent",
        {"confirmation": vault_id, "reason": "remove acceptance fixture"},
        cookie=cookie, csrf=csrf, expected=(204,)
    )
    client.request("GET", "/api/v1/vault", bearer=secret, expected=(401,))
    _, audit, _ = client.request("GET", "/api/v1/admin/audit-log", cookie=cookie)
    actions = {item["action"] for item in audit["items"] if item["vault_id"] == vault_id}
    assert {"view_content", "suspend", "resume", "soft_delete", "restore", "permanent_delete"} <= actions
    print("live acceptance: PASS (temporary Vault permanently removed)")


if __name__ == "__main__":
    main()
