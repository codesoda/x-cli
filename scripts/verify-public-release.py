#!/usr/bin/env python3
"""Explicit live public smoke test of a supplied executable, never browser credentials."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time

ROOT = "20"
PARENT = "1903105387932295260"
CHILD = "1903106713588568359"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--expect-version", required=True)
    parser.add_argument("--live-public", action="store_true")
    args = parser.parse_args()
    if sys.flags.optimize:
        parser.error("Run without Python optimization so verification assertions are enabled")
    if not args.live_public:
        parser.error("Explicit --live-public is required; this contacts FxTwitter")
    binary = args.binary.resolve(strict=True)
    started = int(time.time())
    checks = []
    with tempfile.TemporaryDirectory(prefix="xcli-public-smoke-") as directory:
        state = Path(directory).resolve() / "state"

        def run(name, argv, exit_code=0, human=False):
            result = subprocess.run(
                [str(binary), "--data-dir", str(state), *argv],
                capture_output=True, text=True, timeout=180, check=False,
            )
            if result.returncode != exit_code:
                # Do not log returned content. Stop immediately on failure/rate limit.
                raise RuntimeError(f"{name}: expected exit {exit_code}, got {result.returncode}; payload withheld")
            value = result.stdout if human else json.loads(result.stdout or result.stderr)
            checks.append(name)
            return value

        def public(output, cache):
            assert output["provenance"]["backend"] == "fxtwitter"
            assert output["provenance"]["account_id"] is None
            assert output["provenance"]["cache"] == cache
            assert started <= output["provenance"]["retrieved_at"] <= int(time.time())
            assert output["provenance"]["age_seconds"] >= 0
            assert output["request_failed"] is False
            assert all(isinstance(post["id"], str) for post in output["posts"])
            return output

        version = run("installed_version", ["--version"], human=True).strip()
        assert version == f"xcli {args.expect_version}"
        read = public(run("read_id_json", ["read", ROOT, "--backend", "fxtwitter"]), "miss")
        assert read["posts"][0]["id"] == ROOT
        assert read["posts"][0]["text"] == "just setting up my twttr"
        assert read["complete"] and read["posts"][0]["parent_known"]
        for name, url in [
            ("read_x_url_cache_hit", "https://x.com/jack/status/20?s=1"),
            ("read_twitter_url_cache_hit", "https://twitter.com/jack/status/20"),
        ]:
            hit = public(run(name, ["read", url]), "hit")
            assert hit["posts"] == read["posts"]
            assert hit["provenance"]["retrieved_at"] == read["provenance"]["retrieved_at"]
        text = run("human_output", ["read", ROOT, "--human"], human=True)
        assert "@jack" in text and "just setting up my twttr" in text
        content = state / "cache" / "content"
        before = {p.name: p.read_bytes() for p in content.iterdir()}
        public(run("no_cache_read", ["read", ROOT, "--no-cache"]), "miss")
        assert before == {p.name: p.read_bytes() for p in content.iterdir()}
        public(run("refresh_read", ["read", ROOT, "--refresh"]), "miss")
        public(run("zero_ttl_read", ["read", ROOT, "--cache-ttl", "0"]), "miss")
        root = public(run("root_thread", ["thread", ROOT, "--no-cache"]), "miss")
        assert root["complete"] and root["parent_chain_complete"]
        assert root["stop_reason"] == "root_reached"
        partial = public(run("bounded_parent_chain", ["thread", CHILD, "--max-parents", "1", "--no-cache"], 12), "miss")
        assert [p["id"] for p in partial["posts"]] == [PARENT, CHILD]
        assert not partial["complete"] and not partial["parent_chain_complete"]
        assert partial["stop_reason"] == "parent_limit"
        full = public(run("complete_parent_chain", ["thread", CHILD, "--max-parents", "5", "--no-cache"]), "miss")
        assert [p["id"] for p in full["posts"]] == [ROOT, PARENT, CHILD]
        assert full["complete"] and full["parent_chain_complete"]
        assert full["posts"][2]["parent_id"] == PARENT and full["posts"][1]["parent_id"] == ROOT
        invalid = run("invalid_url_rejected", ["read", "https://example.invalid/status/20"], 2)
        assert invalid["kind"] == "invalid_input"
        incompatible = run("public_account_combination_rejected", ["read", ROOT, "--backend", "fxtwitter", "--account", "unused"], 7)
        assert incompatible["kind"] == "unsupported"
        doctor = run("local_doctor", ["doctor"])
        assert doctor["connections"] == 0 and doctor["checks"]["keychain"] == "not accessed"
        purge = run("cache_purge", ["cache", "purge"])
        assert purge["purged"] and purge["rate_limit_cooldowns_preserved"]
        assert list(content.iterdir()) == []
        public(run("fresh_read_after_purge", ["read", ROOT]), "miss")
    print(json.dumps({
        "binary": str(binary), "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "version": version, "started_at": started, "finished_at": int(time.time()),
        "backend": "fxtwitter", "browser_access": False, "checks_passed": checks,
    }, indent=2))


if __name__ == "__main__":
    main()
