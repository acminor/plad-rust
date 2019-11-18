#!/bin/bash

source ~/.zshenv > /dev/null
source ./src_env.sh > /dev/null

#TEMPS="data/templates_no_ok.toml"
#TEMPS="data/template_no_ok.toml"
TEMPS="data/templates__nfd_def.toml"
#TEMPS="data/templates__nfd_def_full.toml"
#TEMPS="data/templates_no_log_nfd_def.toml"

#DATA="/data/star_extra_data/star_dataset/data/gwac"
DATA="/home/austin/research/star_subset"

function safe_call {
    if [[ $1 == "" ]]
    then
        opt=""
    else
        opt="--$1"
    fi

    # fragment should cut time to run by x
    RUST_BACKTRACE=1 cargo run $opt --target-dir=/data/tmp/build_artifacts/match_filter --\
                  --input ${DATA} \
                  --templates-file ${TEMPS} \
                  --noise .06 \
                  --rho 4.0 \
                  --window-length $2 \
                  --skip-delta 15 \
                  --fragment 1 \
                  --alert-threshold 200 #0.05 #15.1 #8.1
}

case $1 in
    "release")
        safe_call $1 $2
        ;;
    "debug")
        safe_call "" $2
        ;;
    *)
        echo "Must use either release or debug as options."
        ;;
esac
