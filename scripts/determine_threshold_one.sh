#!/bin/bash

source ~/.zshenv
source src_env.sh

TEMPS="data/templates__nfd_def.toml"
DATA="~/Data/preflare2.db"
CMD=$(cat <<EOF
./target/release/match_filter
          --input ${DATA} --templates-file ${TEMPS} --noise .06 --rho 4.0
          --window-length $1 --skip-delta 15 --fragment 1
          --tartan-test true
          --tartan-test-file ~/Code/tartan/noised_star_gen/noise_types/plain_signal.toml
          --plot false
          $2 $3 $4 $5 $6 --alert-threshold {}
EOF
)

cargo build
python3 scripts/determine_threshold.py \
        $CMD
