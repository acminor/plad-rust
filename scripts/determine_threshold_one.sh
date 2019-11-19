#!/bin/bash

source ~/.zshenv
source src_env.sh

python3 scripts/determine_threshold.py \
        --data-dir "/home/austin/research/microlensing_star_data/star_subset" \
        --build-dir "/data/tmp/build_artifacts/match_filter" \
        --templates "data/templates__nfd_def.toml" \
        --window-length $1
