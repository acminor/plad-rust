#!/bin/bash

source ~/.zshenv
source src_env.sh

TEMPS="data/templates__nfd_def.toml"
DATA="/home/austin/research/microlensing_star_data/star_subset"

python3 scripts/determine_threshold.py \
        --target-dir /data/tmp/build_artifacts/match_filter \
        --input ${DATA} \
        --templates-file ${TEMPS} \
        --noise .06 \
        --rho 4.0 \
        --window-length $1 \
        --skip-delta 15 \
        --fragment 1 \
        $2 $3 $4 $5 $6
