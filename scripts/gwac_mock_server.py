import os
import click
import time
import toml
import msgpack
import sqlite3 as sq3
from pathlib import Path

class Star:
    def __init__(self, uid, samples):
        self.uid = uid
        self.samples = samples

class TX:
    def __init__(self, filename, nfd_flip):
        os.mkfifo(filename)
        self.pipe = open(filename, 'w')
        self._filename = filename

        # -1 is because NFD GWAC data is upside down
        if nfd_flip:
            self._scale = -1
        else:
            self._scale = 1
    # NOTE originally used __del__ method but was not guaranteed to be called
    # following resource below to implement a context manager
    # https://stackoverflow.com/questions/865115/how-do-i-correctly-clean-up-a-python-object
    def __enter__(self):
        return self
    def __exit__(self, exc_type, exc_value, traceback):
        os.unlink(self._filename)
    def start_frame(self):
        self.pipe.write("start\n")
    def file_name(self):
        #self.pipe.write(str(time.process_time())+'_gwac.gwac.star\n')
        # NOTE unsure what first three for but last
        #      three are for H:M:S time code (from what I understand of the code)
        cur_time = time.gmtime()
        self.pipe.write('{}_{}_{}_{}_{}_{}'.format(
            'gwac', 'gwac', 'gwac', cur_time.tm_hour, cur_time.tm_min, cur_time.tm_sec))
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
            str(float(star.samples[sample_point]) * self._scale), # mag {float}
            str(sample_point), # NOTE changed to logical-time for testing purposes (not real-time)
            #str(sample_point*15.0), # timestamp {float} [15 second sampling]
            str(59.6), # elliptticity {float}
            'rnd_ccd', # ccd_num {string}
        ]) + '\n'

        self.pipe.write(msg)

@click.command()
@click.argument('gwac_filename',
                type=click.Path(dir_okay=True, writable=True, readable=True))
@click.argument('data_dir', type=click.Path(readable=True))
@click.option('--skip-confirm', type=bool, default=False)
@click.option('--start-time', type=int, default=-1) # for nfd (or other history using methods)
@click.option('--end-time', type=int, default=-1) # for nfd (or other history using methods)
@click.option('--use-tartan-pre-duration', type=bool, default=False) # for nfd (or other history using methods)
def main(gwac_filename, data_dir, skip_confirm, **kwargs):
    i = 0
    max_len = -1
    stars = list()

    if Path(data_dir).suffix == '.db': # Tartan Stars (new fmt)
        with sq3.connect(data_dir) as db:
            db.row_factory = sq3.Row
            schema = db.execute('select * from GenSchema;').fetchone()
            schema = toml.loads(schema['schema'])

            # cache data (for nfd or other history using methods)
            tartan_start_tm = 0
            tartan_end_tm = schema['signal']['start_len']

            i = 0
            for star in db.execute('select * from StarEntry;').fetchall():
                desc = toml.loads(star['desc'])
                data = msgpack.unpackb(star['data'])

                max_len = len(data) if len(data) > max_len else max_len

                stars.append(Star(desc['id'], data))
                i += 1
                if i % 10 == 0:
                    print('Still loading stars...')

            nfd_flip = False
    else: # NFD Stars
        for file in Path(data_dir).glob('*.dat'):
            # str(file) is for issues running with pypy3
            with open(str(file)) as star_data:
                data = list()
                for line in star_data.readlines():
                    inner_data = line.split()
                    data.append(inner_data[1])
            max_len = len(data) if len(data) > max_len else max_len
            stars.append(Star(str(file), data))
            i += 1
            if i % 10 == 0:
                print("Still loading stars...")
        nfd_flip = True

    start_tm = 0
    if kwargs['start_time'] != -1:
        start_tm = kwargs['start_time']

    if kwargs['use_tartan_pre_duration']:
        start_tm = tartan_start_tm
        max_len = tartan_end_tm
    elif kwargs['end_time'] != -1:
        max_len = kwargs['end_time']

    print('Number of Stars: {}'.format(len(stars)))
    print('Number of Tx stages: {}'.format(max_len))

    if not skip_confirm:
        while not click.confirm('Data loaded: do you want to start?'):
            pass

    max_len = len(max(stars, key=lambda s: len(s.samples)).samples)
    with TX(gwac_filename, nfd_flip) as tx:
        for i in range(start_tm, max_len):
            tx.start_frame()
            tx.file_name()
            for star in stars:
                if i < len(star.samples):
                    tx.star(star, i)
            tx.end_frame()

if __name__ == '__main__':
    main()
