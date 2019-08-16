import time
import zmq
import socket

import capnp
capnp.remove_import_hook()
predictor = capnp.load('../protos/predictor.capnp')

#zmq_context = zmq.Context()
#socket = zmq_context.socket(zmq.REQ)
#socket.connect('ipc://test-server')
#socket.send(b'test')

s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect('testing2')
sock = '127.0.0.1:12345'
sock = s
client = capnp.TwoPartyClient(sock)
pred = client.ez_restore('predictor').cast_as(predictor.Predictor)

for i in range(0, 1):
    rq = pred.init_request()
    rq.predictor = 'npp'
    rq.args.entries = [
        {'key': 'look_back', 'val': '1'},
        {'key': 'arima_model_file',
        'val': '/home/austin/libraries/rust_stuff'
        +'/match_filter/data/stars/model_file'},
    ]
    pm = rq.send()
    pm_res = pm.wait()
    uid = pm_res.uid
    print(str(pm_res))

TEST_LEN = 10000
promises = list()

start_time = time.time()
for i in range(0, TEST_LEN):
    rq = pred.predict_request()
    rq.req.lookBacks = [[1],[2]]
    rq.req.times = [1,2]
    rq.req.predictorUID = uid
    pm = rq.send()
    promises.append(pm)

for pm in promises:
    _ = pm.wait()
end_time = time.time()

print('FINISHED: {}s'.format(end_time-start_time))
print('PER: {}s'.format((end_time-start_time)/TEST_LEN))
