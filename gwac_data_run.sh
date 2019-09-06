#!/bin/bash

source ~/.zshenv > /dev/null
source ./src_env.sh > /dev/null

function safe_call {
    if [[ $1 == "" ]]
    then
        opt=""
    else
        opt="--$1"
    fi

    RUST_BACKTRACE=1 cargo run $opt --\
                  --input /data/star_extra_data/star_dataset/data/gwac \
                  --templates-file \
                    data/templates-nfd.toml \
                  --noise .06 \
                  --rho 4.0 \
                  --window-length $2
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
