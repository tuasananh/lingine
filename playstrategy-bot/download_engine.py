import os
import sys
import json
import urllib.request

REPO = "tuasananh/lingine"
API_URL = f"https://api.github.com/repos/{REPO}/releases/latest"


def parse_config_fallback(config_path):
    """
    Fallback parser for config.yml in case yaml library is not available.
    """
    engine_dir = "../target/release/"
    engine_name = "lingine"

    if not os.path.exists(config_path):
        return engine_dir, engine_name

    try:
        with open(config_path, "r") as f:
            lines = f.readlines()

        in_engine_section = False
        for line in lines:
            stripped = line.strip()
            if not stripped or stripped.startswith("#"):
                continue

            # Simple indentation check to see if we are in/out of engine section
            if line.startswith("engine:"):
                in_engine_section = True
                continue
            elif (
                in_engine_section
                and not line.startswith(" ")
                and not line.startswith("\t")
            ):
                in_engine_section = False

            if in_engine_section:
                if ":" in stripped:
                    key, val = stripped.split(":", 1)
                    key = key.strip()
                    val = val.split("#")[0].strip().strip('"').strip("'")
                    if key == "dir":
                        engine_dir = val
                    elif key == "name":
                        engine_name = val
    except Exception as e:
        print(f"Warning: Fallback config parser failed: {e}")

    return engine_dir, engine_name


def get_engine_config():
    config_path = os.path.join(os.path.dirname(__file__), "config.yml")

    try:
        import yaml

        with open(config_path, "r") as f:
            config = yaml.safe_load(f)
        engine_config = config.get("engine", {})
        engine_dir = engine_config.get("dir", "../target/release/")
        engine_name = engine_config.get("name", "lingine")
        return engine_dir, engine_name
    except ImportError:
        print("yaml library not found, using fallback parser...")
        return parse_config_fallback(config_path)
    except Exception as e:
        print(f"Error parsing config.yml with yaml: {e}, using fallback parser...")
        return parse_config_fallback(config_path)


def main():
    engine_dir_rel, engine_name = get_engine_config()

    # Resolve the engine directory relative to the directory of this script
    script_dir = os.path.dirname(os.path.abspath(__file__))
    engine_dir = os.path.abspath(os.path.join(script_dir, engine_dir_rel))
    engine_path = os.path.join(engine_dir, engine_name)

    print(f"Engine target path: {engine_path}")
    print("Checking for the latest lingine release on GitHub...")

    req = urllib.request.Request(
        API_URL,
        headers={
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) lingine-updater"
        },
    )

    try:
        with urllib.request.urlopen(req) as response:
            data = json.loads(response.read().decode())
    except Exception as e:
        print(f"Error fetching latest release from GitHub: {e}", file=sys.stderr)
        sys.exit(1)

    tag_name = data.get("tag_name")
    print(f"Latest release found: {tag_name}")

    assets = data.get("assets", [])
    download_url = None
    for asset in assets:
        name = asset.get("name", "")
        if "linux-x86_64" in name:
            download_url = asset.get("browser_download_url")
            print(f"Found linux-x86_64 asset: {name}")
            break

    if not download_url:
        print(
            "Error: Could not find a linux-x86_64 asset in the latest release.",
            file=sys.stderr,
        )
        sys.exit(1)

    # Create target directory
    os.makedirs(engine_dir, exist_ok=True)

    print(f"Downloading engine from {download_url}...")
    try:
        download_req = urllib.request.Request(
            download_url,
            headers={
                "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) lingine-updater"
            },
        )
        with (
            urllib.request.urlopen(download_req) as response,
            open(engine_path, "wb") as out_file,
        ):
            out_file.write(response.read())
        print(f"Engine downloaded successfully to {engine_path}")
    except Exception as e:
        print(f"Error downloading engine: {e}", file=sys.stderr)
        sys.exit(1)

    # Make engine executable
    try:
        os.chmod(engine_path, 0o755)
        print("Engine made executable (chmod +x).")
    except Exception as e:
        print(f"Warning: Could not make engine executable: {e}", file=sys.stderr)


if __name__ == "__main__":
    main()
