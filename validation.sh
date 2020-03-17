#!/bin/bash

source ~/.zshenv > /dev/null
source ./src_env.sh > /dev/null

#TEMPS="data/template_no_ok.toml"
#TEMPS="data/templates_no_ok.toml"
#TEMPS="data/templates__nfd_def.toml"
#TEMPS="/home/austin/Data/templates/templates-1800.0x87616.0-25x25.toml"
#TEMPS="/home/austin/Data/templates/templates-1800.0x87616.0-30x30.toml"
TEMPS="/home/austin/Data/validation/valid_temps_2.toml"
DATA="/home/austin/Data/validation/star2"

function safe_call {
    if [[ $1 == "" ]]
    then
        opt=""
    else
        opt="--$1"
    fi

    # fragment should cut time to run by x
    RUST_BACKTRACE=1 cargo run $opt --\
                  --input ${DATA} \
                  --templates-file ${TEMPS} \
                  --noise .06 \
                  --rho 4.0 \
                  --window-length 30 \
                  --skip-delta 30 \
                  --fragment 1 \
                  --plot true \
                  --alert-threshold 3700 \
                  --star_group_sz 256 \
                  --template_group_sz 256 \
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
