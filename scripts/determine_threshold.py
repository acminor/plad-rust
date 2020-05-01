import re
import subprocess
import click
import toml

def construct_cmd(cmd, threshold):
    cmd = cmd.format(threshold)
    print(cmd)
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
@click.option('--output-file', required=True,
              help='File in which to store results of tuning.')
@click.argument('cmd', required=True, nargs=-1)
                #help='Python fmt string for command with fmt for threshold.')
def main(output_file, cmd):
    cmd = ' '.join(cmd)
    print(cmd)

    results = []

    alert_threshold_window = [0.0, 2000.0]
    alert_threshold = 0.0
    alert_threshold_prev = 1000.0
    alert_threshold_original_max = 2000.0

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

        # in the case where the false negatives are at the boundary
        # will lead to a slow stall in the final adjustment loop
        # as the threshold slowly increases to find the proper value
        # NOTE should mention this in thesis
        if abs(alert_threshold - alert_threshold_original_max) < 5.0:
            alert_threshold_original_max *= 2
            alert_threshold_window = (
                    alert_threshold_window[0],
                    alert_threshold_original_max
            )


        alert_threshold_prev = alert_threshold
        alert_threshold = (alert_threshold_window[0] + alert_threshold_window[1])/2.0

        proc = subprocess.run(
            construct_cmd(cmd, alert_threshold),
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

        results.append({
            'alert_threshold': alert_threshold,
            'adp_stats': adp,
            'detection_stats': pos,
        })

    while pos['num_false_events'] > 0:
        alert_threshold += 0.0001

        proc = subprocess.run(
            construct_cmd(cmd, alert_threshold),
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, shell=True, encoding='utf8'
        )
        output = proc.stdout
        adp, pos = parse_results(output)

        results.append({
            'alert_threshold': alert_threshold,
            'adp_stats': adp,
            'detection_stats': pos,
        })

        print("Alert Threshold: {}".format(alert_threshold))
        print("ADP Stats: {}".format(adp))
        print("Positives Stats: {}".format(pos))

    with open(output_file, 'w+') as file:
        toml.dump({
            'results': results
        }, file)

if __name__ == '__main__':
    main()
