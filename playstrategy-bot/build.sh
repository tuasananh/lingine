#!/usr/bin/env bash
# exit on error
set -o errexit

# Install requirements
pip install -r requirements.txt

# Download latest engine release
python download_engine.py
