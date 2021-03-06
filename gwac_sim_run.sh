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

    # fragment should cut time to run by x
    RUST_BACKTRACE=0 cargo run $opt --\
                  --gwac-file $3 \
                  --templates-file ${TEMPS} \
                  --noise .06 \
                  --rho 4.0 \
                  --window-length $2 \
                  --skip-delta 30 \
                  --fragment 1 \
                  --alert-threshold 15.1 #8.1
}

case $1 in
    "release")
        safe_call $1 $2 $3
        ;;
    "debug")
        safe_call "" $2 $3
        ;;
    *)
        echo "Must use either release or debug as options."
        ;;
esac
