#!/bin/bash

source ~/.zshenv
source src_env.sh

python3 scripts/determine_threshold.py \
        --data-dir "/home/austin/research/microlensing_star_data/gaussian_generated" \
        --build-dir "/data/tmp/build_artifacts/match_filter" \
        --templates "data/templates__nfd_def.toml" \
        --tartan-file "/home/austin/research/tartan/noised_star_gen/noise_types/reduced_gaussian.toml" \
        --window-length $1
