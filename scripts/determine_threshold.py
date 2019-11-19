import re
import subprocess
import click

def construct_cmd(is_release, data_dir, build_dir, templates, window_length, alert_threshold):
    cmd = '''
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
        build_dir, #"/data/tmp/build_artifacts/match_filter",
        data_dir,
        templates, #"data/templates__nfd_def.toml",
        window_length,
        alert_threshold
    )

    return cmd

def parse_results(output):
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

    adp_regex = r'^.*ADP stats:, std_dev: (.*), avg: (.*), max: (.*), min: (.*)$'
    adp_regex = re.compile(adp_regex)
    pos_regex = r'^.*num_stars: (.*), num_false_events: (.*), num_true_events: (.*), num_events.*$'
    pos_regex = re.compile(pos_regex)

    adp = {}
    pos = {}
    for line in output.splitlines():
        adps = adp_regex.findall(line)
        poss = pos_regex.findall(line)

        if len(adps) != 0:
            adps = adps[0]
            adp = {
                'std_dev': float(adps[0]),
                'avg': float(adps[1]),
                'max': float(adps[2]),
                'min': float(adps[3]),
            }
        elif len(poss) != 0:
            poss = poss[0]
            pos = {
                'num_stars': int(poss[0]),
                'num_false_events': int(poss[1]),
                'num_true_events': int(poss[2]),
                'num_events': int(poss[1]) + int(poss[2]),
            }
    return adp, pos


@click.command()
@click.option('--data-dir', required=True,
              type=click.Path(exists=True, dir_okay=True, readable=True))
@click.option('--build-dir', required=True,
                type=click.Path(dir_okay=True, writable=True, readable=True))
@click.option('--templates', required=True,
                type=click.Path(exists=True, file_okay=True, readable=True))
@click.option('--window-length', required=True, type=int)
def main(data_dir, build_dir, templates, window_length):
    # NOTE for now, always assume release for testing
    is_release = True
    #data_dir = "/home/austin/research/microlensing_star_data/star_subset"
    #window_length = 30

    alert_threshold_window = [0.0, 200.0]
    alert_threshold = 0.0
    alert_threshold_prev = 100.0

    pos = None
    adp = None
    # converge on optimal value
    while abs(alert_threshold - alert_threshold_prev) > 0.001:
        # do a binary search like search for finding optimal value
        # -- not useful for simple case -- but good for when we integrate reject testing
        if pos and pos['num_false_events'] > 0:
            alert_threshold_window = (alert_threshold, alert_threshold_window[1])
        elif pos:
            alert_threshold_window = (alert_threshold_window[0], alert_threshold)


        alert_threshold_prev = alert_threshold
        alert_threshold = (alert_threshold_window[0] + alert_threshold_window[1])/2.0

        proc = subprocess.run(
            construct_cmd(is_release, data_dir, build_dir,
                          templates, window_length, alert_threshold),
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, shell=True, encoding='utf8'
        )
        output = proc.stdout
        adp, pos = parse_results(output)

        print("Alert Threshold: {}".format(alert_threshold))
        print("ADP Stats: {}".format(adp))
        print("Positives Stats: {}".format(pos))

    while pos['num_false_events'] > 0:
        alert_threshold += 0.0001

        proc = subprocess.run(
            construct_cmd(is_release, data_dir, window_length, alert_threshold),
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, shell=True, encoding='utf8'
        )
        output = proc.stdout
        adp, pos = parse_results(output)

        print("Alert Threshold: {}".format(alert_threshold))
        print("ADP Stats: {}".format(adp))
        print("Positives Stats: {}".format(pos))

if __name__ == '__main__':
    main()
