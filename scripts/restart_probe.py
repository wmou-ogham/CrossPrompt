#!/usr/bin/env python3
"""Verify that an admin session and audit data survive a Compose restart."""

import argparse
import json
import os

from live_acceptance import Client


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("before", "after"))
    parser.add_argument("--base-url", default="http://127.0.0.1:18080")
    parser.add_argument("--admin-password-file")
    parser.add_argument("--state-file", required=True)
    args = parser.parse_args()
    client = Client(args.base_url)

    if args.mode == "before":
        if not args.admin_password_file:
            parser.error("--admin-password-file is required in before mode")
        password = open(args.admin_password_file, encoding="utf-8").read().strip()
        _, login, headers = client.request(
            "POST", "/api/v1/admin/session", {"username": "admin", "password": password}
        )
        cookie = headers.get("Set-Cookie").split(";", 1)[0]
        _, audit, _ = client.request("GET", "/api/v1/admin/audit-log", cookie=cookie)
        highest_audit_id = max((item["id"] for item in audit["items"]), default=0)
        descriptor = os.open(args.state_file, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump({"cookie": cookie, "csrf": login["csrf_token"], "highest_audit_id": highest_audit_id}, handle)
        print("restart probe: state captured")
        return

    try:
        state = json.load(open(args.state_file, encoding="utf-8"))
        client.request("GET", "/api/v1/admin/session", cookie=state["cookie"])
        _, audit, _ = client.request("GET", "/api/v1/admin/audit-log", cookie=state["cookie"])
        ids = {item["id"] for item in audit["items"]}
        if state["highest_audit_id"] and state["highest_audit_id"] not in ids:
            raise AssertionError("pre-restart audit entry was not found")
        print("restart probe: PASS (session and audit survived)")
    finally:
        os.remove(args.state_file)


if __name__ == "__main__":
    main()
