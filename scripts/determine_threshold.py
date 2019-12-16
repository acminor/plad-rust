import re
import subprocess
import click

def construct_cmd(is_release, target_dir, alert_threshold, match_filter_opts):
    cmd = '''
    cargo run {} --target-dir={} -- \
    --alert-threshold={}\
    --plot=false\
    {}
    '''.format(
        "--release "if is_release else "",
        target_dir,
        str(alert_threshold),
        ' '.join(list(match_filter_opts)),
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


@click.command(context_settings=dict(
    ignore_unknown_options=True,
))
@click.option('--target-dir', required=True)
@click.argument('match_filter_opts', nargs=-1)
def main(target_dir, match_filter_opts):
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
            construct_cmd(is_release, target_dir, alert_threshold, match_filter_opts),
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, shell=True, encoding='utf8'
        )
        output = proc.stdout
        adp, pos = parse_results(output)

        # NOTE: error occurred b/c output is not good
        if len(adp) == 0:
            print(output)
            exit(-1)

        print("Alert Threshold: {}".format(alert_threshold))
        print("ADP Stats: {}".format(adp))
        print("Positives Stats: {}".format(pos))

    while pos['num_false_events'] > 0:
        alert_threshold += 0.0001

        proc = subprocess.run(
            construct_cmd(is_release, target_dir, alert_threshold, match_filter_opts),
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, shell=True, encoding='utf8'
        )
        output = proc.stdout
        adp, pos = parse_results(output)

        print("Alert Threshold: {}".format(alert_threshold))
        print("ADP Stats: {}".format(adp))
        print("Positives Stats: {}".format(pos))

if __name__ == '__main__':
    main()
