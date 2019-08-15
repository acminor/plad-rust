import typing as ty
import dataclasses as dc
import logging as log
log.basicConfig(level=log.DEBUG)

import capnp
capnp.remove_import_hook()

import lstm

predictor = capnp.load('../protos/predictor.capnp')

temp = predictor.Predictor.PredictRequest.new_message()

temp.lookBacks = [[i for i in range(0,2)] for j in range(0,2)]
temp.times = [i for i in range(0,2)]
temp.predictorUID = 0

print('pred:'+str(temp.to_bytes()))

def map_to_dict(mapping):
    return { ent.key: ent.val for ent in mapping.entries }

@dc.dataclass
class PersistantState:
    init_number: int = -1
    state_map: ty.Dict[int, ty.Any] = dc.field(default_factory=dict)

ps = PersistantState()

class NullPredictor(predictor.Predictor.Server):
    def predict(self, req, **kwargs):
        global ps
        log.debug('Called Predictor.predict with: '+str(req))

        res = predictor.Predictor.PredictResponse.new_message()
        predictions = ps.state_map[req.predictorUID].predict(
            req.lookBacks, req.times)
        res.init('predictions', len(predictions))
        for i in range(0, len(predictions)):
            res.predictions[i] = float(predictions[i])

        return res
    def init(self, predictor, args, **kwargs):
        global ps
        log.debug('Called Predictor.init with: '\
                  + str((str(predictor), str(args))))

        ps.init_number += 1
        log.info('Predictor.init init_number: {}'.format(ps.init_number))

        args = map_to_dict(args)
        ps.state_map[ps.init_number] = lstm.build_predictor(**args)
        log.debug('Predictor.init state_map len: {}'.format(
            len(ps.state_map)))

        res = ps.init_number
        return res

def restore(ref):
    assert ref.as_text() == 'predictor'
    return NullPredictor()

server = capnp.TwoPartyServer('127.0.0.1:12345', restore)
server.run_forever()
