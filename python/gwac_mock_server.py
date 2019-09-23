import os
import click
import time
from pathlib import Path

class Star:
    def __init__(self, uid, samples):
        self.uid = uid
        self.samples = samples

class TX:
    def __init__(self, filename):
        os.mkfifo(filename)
        self.pipe = open(filename, 'w')
        self._filename = filename
    def __del__(self):
        os.unlink(self._filename)
    def start_frame(self):
        self.pipe.write("start\n")
    def file_name(self):
        self.pipe.write(str(time.process_time())+'.gwac.star\n')
    def end_frame(self):
        self.pipe.write("end\n")
    def star(self, star, sample_point):
        msg = ' '.join([
            str(64.1), # xpix {float}
            str(63.2), # ypix {float}
            str(62.3), # ra {float}
            str(61.4), # dec {float}
            'zone_defense', # zone {string}
            star.uid, # star id {string}
            star.samples[sample_point], # mag {float}
            str(sample_point*15.0), # timestamp {float} [15 second sampling]
            str(59.6), # elliptticity {float}
            'rnd_ccd', # ccd_num {string}
        ]) + '\n'

        self.pipe.write(msg)

@click.command()
@click.argument('gwac_filename',
                type=click.Path(dir_okay=True, writable=True, readable=True))
@click.argument('data_dir', type=click.Path(dir_okay=True, readable=True))
def main(gwac_filename, data_dir):
    stars = list()
    for file in Path(data_dir).glob('*.dat'):
        with open(file) as star_data:
            data = list()
            for line in star_data.readlines():
                inner_data = line.split()
                data.append(inner_data[1])
        stars.append(Star(str(file), data))

    max_len = len(max(stars, key=lambda s: len(s.samples)).samples)
    tx = TX(gwac_filename)
    for i in range(0, max_len):
        print((tx, i))
        tx.start_frame()
        tx.file_name()
        for star in stars:
            if i < len(star.samples):
                tx.star(star, i)
        tx.end_frame()

if __name__ == '__main__':
    main()
