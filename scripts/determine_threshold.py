import re
import subprocess
import click

def construct_cmd(is_release, data_dir, window_length, alert_threshold):
    cmd = '''
    source ~/.zshenv
    source src_env.sh

    cargo run {} --target-dir={} --\
    --input={}\
    --templates-file={}\
    --noise=.06\
    --rho=4.0\
    --window-length={}\
    --skip-delta=15\
    --fragment=1\
    --alert-threshold={}\
    --plot=false
    '''.format(
        "--release "if is_release else "",
        "/data/tmp/build_artifacts/match_filter",
        data_dir,
        "data/templates__nfd_def.toml",
        window_length,
        alert_threshold
    )

    return cmd

@click.command()
def main():
    adp_regex = r'^.*ADP stats:, std_dev: (.*), avg: (.*), max: (.*), min: (.*)$'
    adp_regex = re.compile(adp_regex)
    pos_regex = r'^.*num_stars: (.*), num_false_events: (.*), num_true_events: (.*), num_events.*$'
    pos_regex = re.compile(pos_regex)

    is_release = True
    data_dir = "/home/austin/research/microlensing_star_data/star_subset"
    window_length = 30
    alert_threshold = 0.1
    proc = subprocess.run(
        construct_cmd(is_release, data_dir, window_length, alert_threshold),
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, shell=True, encoding='utf8'
    )

    output = proc.stdout

    # NOTE: remove ansi text coloring
    # from https://stackoverflow.com/a/14693789
    # 7-bit C1 ANSI sequences
    ansi_escape = re.compile(r'''
        \x1B    # ESC
        [@-_]   # 7-bit C1 Fe
        [0-?]*  # Parameter bytes
        [ -/]*  # Intermediate bytes
        [@-~]   # Final byte
    ''', re.VERBOSE)
    output = ansi_escape.sub('', output)

    for line in output.splitlines():
        print(adp_regex.findall(line))
        print(pos_regex.findall(line))
    print(output)

if __name__ == '__main__':
    main()
