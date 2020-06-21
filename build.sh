#!/bin/bash

# ansi colors from
# https://www.lihaoyi.com/post/BuildyourownCommandLinewithANSIescapecodes.html#8-colors

ansi_black="\u001b[30m"
ansi_red="\u001b[31m"
ansi_green="\u001b[32m"
ansi_yellow="\u001b[33m"
ansi_blue="\u001b[34m"
ansi_magenta="\u001b[35m"
ansi_cyan="\u001b[36m"
ansi_white="\u001b[37m"
ansi_normal="\u001b[0m"

function banner {
    echo -e "${ansi_green}===== $1 =====${ansi_normal}"
}

function banner2 {
    echo -e "${ansi_magenta}----- $1 -----${ansi_normal}"
}

function bannerlist {
    echo -e "${ansi_cyan}- $1 ${ansi_normal}"
}

function banneryesno {
    echo -e "$1"
    echo "Yes/no?"

    while [ /bin/true ]
    do
        read banneryn_input
        case $banneryn_input in
        #"yes"|"Yes"|"y"|"Y"|"YES")
        yes|Yes|y|Y|YES)
            yesno_state="y"
            return
            ;;
        #"no"|"No"|"n"|"N"|"NO")
        no|No|n|N|NO)
            yesno_state="n"
            return
            ;;
        *)
            ;;
        esac
    done
}

function install_arrayfire {
    banner "Installing Arrayfire"
    bannerlist "This process will install Arrayfire to the /opt directory"
    bannerlist "It will not test the installation and not install Cuda/Opencl"
    bannerlist "It will (optionally) update your ld conf to help compilers in finding the package"

    # Only print documentation
    if [[ $1 = "doc" ]]
    then
        return
    fi

    banner2 "Downloading Arrayfire 3.7.1..."
    af_download_link="https://arrayfire.s3.amazonaws.com/3.7.1/ArrayFire-v3.7.1-1_Linux_x86_64.sh"
    af_file="ArrayFire-v3.7.1-1_Linux_x86_64.sh"

    # TODO replace by generic wget
    if [[ ! -e ${af_file} ]]
    then
        wget ${af_download_link}
    fi

    banner2 "Installing Arrayfire 3.7.1..."
    sudo sh ${af_file} --include-subdir --prefix=/opt

    # TODO implement a option for modifying LD path variable
    banneryesno "Add arrayfire entry to ld.so.conf.d and reload ldconfig?\nRequires Root."

    if [[ ${yesno_state} = "y" ]]
    then
        echo /opt/arrayfire/lib | sudo tee -a /etc/ld.so.conf.d/arrayfire.conf > /dev/null
        sudo ldconfig
    fi
}

function test_arrayfire {
    banner "Testing Arrayfire"
    bannerlist "This process will test the Arrayfire installation"
    bannerlist "It will install Cmake3 for building the examples"
    bannerlist "It assumes that all of the Cuda and OpenCL drivers are installed and configured"
    bannerlist "It will build the examples in a directory in /tmp"

    # Only print documentation
    if [[ $1 = "doc" ]]
    then
        return
    fi

    banneryesno "Install Cmake3 and run tests? Requires root to install Cmake3"

    if [[ ${yesno_state} = "y" ]]
    then
        case ${PLATFORM} in
            Ubuntu)
                sudo apt install -y cmake
                CMAKE=cmake
            ;;
            Fedora)
            ;;
            Centos|Rhel)
                sudo yum install -y cmake3
                CMAKE=cmake3
            ;;
        esac
    else
        return
    fi

    af_examples_dir="/tmp/af_temp_build_examples"

    if [[ -d ${af_examples_dir} ]]
    then
        rm -r ${af_examples_dir}
    fi

    cp -r /opt/arrayfire/share/ArrayFire/examples ${af_examples_dir}

    CURRENT_DIR=${PWD}

    cd ${af_examples_dir}
    mkdir build

    cd build
    ${CMAKE} -DASSETS_DIR:PATH=/tmp .. > /dev/null 2>&1

    banner2 "Building examples..."

    make helloworld_cpu > /dev/null 2>&1
    make helloworld_cuda > /dev/null 2>&1
    make helloworld_opencl > /dev/null 2>&1

    ##################################################
    ##################################################
    ##################################################

    if [[ -e "./helloworld/helloworld_cpu" ]]
    then
        ( ./helloworld/helloworld_cpu || false ) > /dev/null 2>&1
    else
        # set bad return code
        $(exit 1)
    fi

    if [[ ! $? -eq 0 ]]
    then
        bannerlist "CPU build and run failed."
        cat << EOF

----------------------------
# for errors run this script
cd ${af_examples_dir}/build
make helloworld_cpu
----------------------------

EOF
    else
        bannerlist "CPU build and run succeeded"
    fi

    ##################################################
    ##################################################
    ##################################################

    if [[ -e "./helloworld/helloworld_cuda" ]]
    then
        ( ./helloworld/helloworld_cuda || false ) > /dev/null 2>&1
    else
        # set bad return code
        $(exit 1)
    fi

    if [[ ! $? -eq 0 ]]
    then
        bannerlist "Cuda build and run failed."
        cat << EOF

----------------------------
# for errors run this script
cd ${af_examples_dir}/build
make helloworld_opencl
----------------------------

EOF
    else
        bannerlist "Cuda build and run succeeded"
    fi

    ##################################################
    ##################################################
    ##################################################

    if [[ -e "./helloworld/helloworld_opencl" ]]
    then
        # thanks https://stackoverflow.com/a/22051517/9286112
        # for double pipe false prevent sigabrt messages
        ( ./helloworld/helloworld_opencl || false ) > /dev/null 2>&1
    else
        # set bad return code
        $(exit 1)
    fi

    if [[ ! $? -eq 0 ]]
    then
        bannerlist "OpenCL build and run failed."
        cat << EOF

----------------------------
# for errors run this script
cd ${af_examples_dir}/build
make helloworld_cuda
----------------------------

EOF
    else
        bannerlist "OpenCL build and run succeeded"
    fi

    cd ${CURRENT_DIR}
}

function install_rust {
    banner "Installing Rust nightly..."
    bannerlist "Installing rustup to manager Rust"
    bannerlist "Installing a minimal, current Rust nightly install"
    bannerlist "> If there is an issue, try installing a different day's Rust nightly"
    bannerlist '> the next day with the following command `rustup toolchain install nightly`'

    # Only print documentation
    if [[ $1 = "doc" ]]
    then
        return
    fi

    banner2 "Installing rustup and Rust nightly..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain nightly -y

    bannerlist "rustup and Rust installed. Please follow instructions to source environment"
    bannerlist "> Or, you can start a new shell"

    source ~/.bash_profile
    source ~/.bashrc
    source ~/.profile
}

function install_packages {
    banner "Installing necessary packages."
    bannerlist "The following packages will be installed (if needed)"

    ubuntu_pkgs=$(cat << EOF
    build-essential git capnproto
    python-dev python3-dev sqlite3
    google-perftools
EOF
               )

    centos_pkgs=$(cat <<EOF
        git capnproto
        python-devel python3-devel
        sqlite3 gperftools-devel
EOF
)

    case ${PLATFORM} in
    Ubuntu)
        bannerlist "> ${ubuntu_pkgs}"
    ;;
    Fedora)
    ;;
    Centos|Rhel)
        bannerlist "> ${centos_pkgs}"
    ;;
    esac

    # Only print documentation
    if [[ $1 = "doc" ]]
    then
        return
    fi

    banner2 "Installing packages..."

    case ${PLATFORM} in
    Ubuntu)
        sudo apt update
        sudo apt install -y ${ubuntu_pkgs}
    ;;
    Fedora)
    ;;
    Centos|Rhel)
        sudo yum update
        sudo yum groupinstall -y "Development Tools"
        sudo yum install -y ${centos_pkgs}
    ;;
    esac
}

function set_platform {
    banner "Determining the current platform..."
    bannerlist "If the platform is not supported, this script might need to be modified"

    # use PRETTY_NAME as that has version included if want version specific script in future
    platform_info=$(cat /etc/os-release | grep "^PRETTY_NAME" | sed 's/^PRETTY_NAME="\(.*\)"$/\1/')

    case ${platform_info} in
        CentOS*)
            banner2 "Platform is Centos"
            PLATFORM="Centos"
            ;;
        Ubuntu*)
            banner2 "Platform is Ubuntu"
            PLATFORM="Ubuntu"
            ;;
        Red\ Hat*)
            banner2 "Platform is Red Hat"
            PLATFORM="Rhel"
            ;;
        *)
            banner2 "Platform is unrecognized"
            bannerlist "Exiting with error"
            exit 1
            ;;
    esac
}

# TODO need to see if works for centos 8 and not just 7

function main {
    banner "Installing necessary components to build plad-rust"
    echo

    while [[ $# -gt 0 ]]
    do
        param=$1
        case ${param} in
            doc*)
                document_only="doc"
                shift
                ;;
            *)
                shift
                ;;
        esac
    done

    set_platform
    echo

    install_arrayfire ${document_only}
    echo
    test_arrayfire ${document_only}
    echo
    install_packages ${document_only}
    echo
    install_rust ${document_only}
    echo

    banner "Necessary components installed"
    banner "You can now run cargo build"
    banner "Look at build.md for more instructions"
}

main $@
