#!/bin/bash

source ~/.zshenv
source src_env.sh

TEMPS="data/templates__nfd_def.toml"
DATA="~/Data/preflare2.db"
CMD=$(cat <<EOF
/home/wamdm/offline_nfd/offline_status.sh --clear-cache &&
/home/wamdm/offline_nfd/offline_nfd.sh
    --report-to-std
    --nfd-args -nfdThreshold {} end_nfd_args
EOF
   )

python3 scripts/determine_threshold.py \
        $CMD
