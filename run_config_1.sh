#!/bin/bash

source ./src_env.sh > /dev/null
source ~/.zshenv > /dev/null

function safe_call {
    if [[ $1 == "" ]]
    then
        opt=""
    else
        opt="--$1"
    fi

    RUST_BACKTRACE=1 cargo run $opt --\
                  --input data/stars \
                  --templates-file \
                    data/templates-1800.0d87616.0-0.1d50.0-50x50.toml \
                  --noise .06 \
                  --rho 4.0 \
                  --window-length 30
}

case $1 in
    "release")
        safe_call $1
        ;;
    "debug")
        safe_call ""
        ;;
    *)
        echo "Must use either release or debug as options."
        ;;
esac
