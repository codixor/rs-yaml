"""Inspect the public registry and GitHub before publishing the checked-out crate."""
import hashlib
import io
import json
import os
import re
from pathlib import Path
import subprocess
import tarfile
import tomllib
from urllib.error import HTTPError
from urllib.request import Request, urlopen


def fetch(url, *, missing_ok=False, token=None):
    request = Request(url, headers={"User-Agent": "codixor-rs-yaml-release", "Cache-Control": "no-cache"})
    if token:
        request.add_header("Authorization", f"Bearer {token}")
    try:
        with urlopen(request, timeout=30) as response:
            return response.read()
    except HTTPError as error:
        if missing_ok and error.code == 404:
            return None
        raise


def registry_commit(name, version):
    metadata = fetch(f"https://crates.io/api/v1/crates/{name}/{version}", missing_ok=True)
    if metadata is None:
        return None
    published = json.loads(metadata)["version"]
    if published["yanked"]:
        raise SystemExit(f"{name} {version} has been yanked")
    archive = fetch(f"https://static.crates.io/crates/{name}/{name}-{version}.crate")
    if hashlib.sha256(archive).hexdigest() != published["checksum"]:
        raise SystemExit("Registry archive checksum mismatch")
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as crate:
        with crate.extractfile(f"{name}-{version}/.cargo_vcs_info.json") as info:
            git = json.load(info)["git"]
    if git.get("dirty"):
        raise SystemExit("Registry archive was built from a modified working tree")
    if not re.fullmatch(r"[0-9a-f]{40}", git["sha1"]):
        raise SystemExit("Registry archive has an invalid Git commit")
    return git["sha1"]


def inspect():
    with Path("Cargo.toml").open("rb") as manifest:
        root = tomllib.load(manifest)
    workspace = root["workspace"]
    version = workspace["package"]["version"]
    # The manifest lists the forks in publish order, starting with the shared core.
    packages = []
    for member in workspace["members"]:
        with (Path(member) / "Cargo.toml").open("rb") as manifest:
            package = tomllib.load(manifest)["package"]
        alias = Path(member).name
        dependency = workspace["dependencies"][alias]
        if (dependency["package"] != package["name"] or dependency["version"] != f"={version}"
                or package["version"] != {"workspace": True}):
            raise SystemExit(f"{member} must use the shared release version {version}")
        packages.append(package["name"])
    if root["package"]["version"] != {"workspace": True}:
        raise SystemExit("rs-yaml must use the shared release version")
    packages.append(root["package"]["name"])
    tag = f"v{version}"
    tagged_push = os.environ["GITHUB_REF_TYPE"] == "tag"
    if tagged_push and os.environ["GITHUB_REF_NAME"] != tag:
        raise SystemExit(f"Push tag must match Cargo.toml ({tag})")

    head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    uploaded = {name: registry_commit(name, version) for name in packages}
    pending = [name for name, commit in uploaded.items() if commit is None]
    commits = {commit for commit in uploaded.values() if commit}
    if len(commits) > 1:
        raise SystemExit(f"Published members of {tag} have different source commits")
    target = next(iter(commits), head)
    if target != head and (tagged_push or pending):
        raise SystemExit(f"Finish the release at its original checked commit {target}")
    if commits:
        subprocess.run(["git", "merge-base", "--is-ancestor", target, head], check=True)

    existing = subprocess.run(
        ["git", "rev-parse", "--verify", "--quiet", f"refs/tags/{tag}^{{commit}}"],
        capture_output=True, text=True,
    )
    if existing.returncode not in (0, 1) or (existing.returncode == 0 and existing.stdout.strip() != target):
        raise SystemExit(f"Existing tag {tag} does not match release commit {target}")

    repo = os.environ["GITHUB_REPOSITORY"]
    release = fetch(f"https://api.github.com/repos/{repo}/releases/tags/{tag}",
                    missing_ok=True, token=os.environ.get("GH_TOKEN"))
    if release is not None and json.loads(release)["draft"]:
        raise SystemExit(f"An existing draft release needs review: {tag}")
    return {
        "version": version,
        "target": target,
        "pending": " ".join(pending),
        "create_release": str(release is None).lower(),
        "required": str(bool(pending) or release is None).lower(),
    }


if __name__ == "__main__":
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
        for key, value in inspect().items():
            print(f"{key}={value}", file=output)
