#!/bin/bash

source ~/.zshenv > /dev/null
source ./src_env.sh > /dev/null

TEMPS="data/templates-1800.0d87616.0-0.1d50.0-600x600.toml"
#DATA="/data/star_extra_data/star_dataset/data/gwac"
DATA="/home/austin/research/star_subset"

function safe_call {
    if [[ $1 == "" ]]
    then
        opt=""
    else
        opt="--$1"
    fi

    RUST_BACKTRACE=1 cargo run $opt --\
                  --input ${DATA} \
                  --templates-file ${TEMPS} \
                  --noise .06 \
                  --rho 4.0 \
                  --window-length $2 \
                  --alert-threshold 1.0
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
