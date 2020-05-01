#!/bin/bash

source ~/.zshenv > /dev/null
source ./src_env.sh > /dev/null

TEMPS="/home/austin/Code/tartan/template_gen/templates-1800.0x87616.0-1x600.toml"
#DATA="/home/austin/Data/nfd_star_dataset_generator/data/gwac"
#DATA="/home/austin/Data/flares.db"
DATA="/home/austin/Data/plain.db"

function safe_call {
    if [[ $1 == "" ]]
    then
        opt=""
    else
        opt="--$1"
    fi

    # fragment should cut time to run by x
    RUST_BACKTRACE=1 cargo build $opt


    time ./target/release/match_filter \
                  --input ${DATA} \
                  --templates-file ${TEMPS} \
                  --noise .06 \
                  --rho 4.0 \
                  --window-length $2 \
                  --skip-delta 1 \
                  --fragment 1 \
		  --plot true \
                  --alert-threshold 260000.0 \
		  $3 $4 $5 $6 $7
}

case $1 in
    "release")
        safe_call $1 $2 $3 $4 $5 $6 $7
        ;;
    "debug")
        safe_call "" $2 $3 $4 $5 $6 $7
        ;;
    *)
        echo "Must use either release or debug as options."
        ;;
esac
